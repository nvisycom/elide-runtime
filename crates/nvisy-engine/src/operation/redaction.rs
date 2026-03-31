//! Redaction operation.
//!
//! Runs at **phase 4** alongside [`GenerateContextOp`]. Evaluates policy
//! rules against detected entities to produce redaction decisions and
//! replacement text.
//!
//! [`GenerateContextOp`]: crate::operation::GenerateContextOp

use nvisy_core::Result;
use nvisy_ontology::entity::{Entities, Entity};
use nvisy_ontology::policy::{PolicyRule, RuleAction, Strategy, TextStrategy};
use nvisy_ontology::provenance::{RedactionDecision, RedactionRecord};
use nvisy_ontology::workflow::Redaction;
use sha2::{Digest, Sha256};

use crate::operation::Operation;
use crate::operation::context::{ParallelContext, SharedContext};
use crate::operation::envelope::PolicyOutcome;

const TARGET: &str = "nvisy_engine::op::redaction";

/// Redaction operation: evaluates policies and produces redaction decisions.
pub struct RedactionOp {
    evaluator: PolicyEvaluator,
}

impl RedactionOp {
    /// Build from graph config and shared context.
    pub async fn new(cfg: &Redaction, shared: &SharedContext) -> Result<Self> {
        let mut rules: Vec<PolicyRule> = shared
            .policies
            .policies
            .iter()
            .flat_map(|p| p.rules.clone())
            .collect();
        rules.sort_by_key(|r| r.priority);

        let evaluator = PolicyEvaluator {
            rules,
            default_spec: Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            default_threshold: cfg.confidence_threshold.unwrap_or(0.5),
        };

        Ok(Self { evaluator })
    }
}

impl Operation for RedactionOp {
    type Input = ParallelContext<Entities>;
    type Output = ParallelContext<PolicyOutcome>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        self.evaluator.call(input).await
    }
}

struct PolicyEvaluator {
    rules: Vec<PolicyRule>,
    default_spec: Strategy,
    default_threshold: f64,
}

impl PolicyEvaluator {
    pub(crate) async fn evaluate(&self, entities: Entities) -> Result<PolicyOutcome> {
        tracing::debug!(
            target: TARGET,
            entity_count = entities.len(),
            rules = self.rules.len(),
            "evaluating policies",
        );
        let mut decisions = Vec::new();
        let mut records = Vec::new();

        for entity in &entities {
            let rule = self.find_matching_rule(entity);

            let (spec, replacement) = match rule {
                Some(r) => match &r.action {
                    RuleAction::Redact { strategy } => {
                        (strategy.clone(), build_replacement(entity, strategy))
                    }
                    action @ (RuleAction::Review
                    | RuleAction::Alert
                    | RuleAction::Block
                    | RuleAction::Suppress
                    | _) => {
                        tracing::debug!(
                            target: TARGET,
                            entity_id = %entity.source.as_uuid(),
                            rule_id = %r.id,
                            action = ?action,
                            "non-redact policy action",
                        );
                        continue;
                    }
                },
                None => {
                    if entity.confidence < self.default_threshold {
                        continue;
                    }
                    (
                        self.default_spec.clone(),
                        build_replacement(entity, &self.default_spec),
                    )
                }
            };

            let entity_id = entity.source.as_uuid();

            let mut decision =
                RedactionDecision::new(entity_id, spec, &replacement, entity.confidence);
            if let Some(r) = rule {
                decision = decision.with_policy_rule_id(r.id);
            }
            decision.source.set_parent_id(Some(entity_id));

            let mut record = RedactionRecord::new(entity_id, &entity.value, entity.confidence);
            if let Some(r) = rule {
                record = record.with_policy_rule_id(r.id);
            }
            record.source.set_parent_id(Some(entity_id));

            tracing::trace!(
                target: TARGET,
                entity_id = %entity_id,
                strategy = ?decision.spec,
                replacement_len = replacement.len(),
                "produced redaction decision",
            );

            decisions.push(decision);
            records.push(record);
        }

        tracing::info!(
            target: TARGET,
            decisions = decisions.len(),
            records = records.len(),
            "policy evaluation complete",
        );

        Ok(PolicyOutcome { decisions, records })
    }

    fn find_matching_rule(&self, entity: &Entity) -> Option<&PolicyRule> {
        self.rules.iter().find(|rule| {
            rule.selector
                .matches(&entity.category, entity.entity_kind, entity.confidence)
        })
    }
}

impl Operation for PolicyEvaluator {
    type Input = ParallelContext<Entities>;
    type Output = ParallelContext<PolicyOutcome>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.evaluate(data)).await
    }
}

/// Produce a replacement string for the given entity and strategy.
fn build_replacement(entity: &Entity, spec: &Strategy) -> String {
    match spec {
        Strategy::Text(text) => build_text_replacement(entity, text),
        Strategy::Image(_) | Strategy::Audio(_) | _ => String::new(),
    }
}

/// Produce a text replacement for the given entity and text strategy.
fn build_text_replacement(entity: &Entity, strategy: &TextStrategy) -> String {
    match strategy {
        TextStrategy::Mask { mask_char } => mask_char.to_string().repeat(entity.value.len()),

        TextStrategy::Replace { placeholder } => {
            if placeholder.is_empty() {
                format!("[{}]", entity.entity_kind.to_string().to_uppercase())
            } else {
                // Never interpolate {value}: it would leak the original.
                placeholder
                    .replace("{entityType}", &entity.entity_kind.to_string())
                    .replace("{category}", &entity.category.to_string())
            }
        }

        TextStrategy::Remove => String::new(),

        TextStrategy::Hash => {
            let mut hasher = Sha256::new();
            hasher.update(entity.value.as_bytes());
            let hash = hasher.finalize();
            let hex: String = hash.iter().take(8).map(|b| format!("{b:02x}")).collect();
            hex
        }

        TextStrategy::Pseudonymize => {
            // Deterministic pseudonym: hash seeded by entity kind + value,
            // producing a consistent identifier for the same input.
            let mut hasher = Sha256::new();
            hasher.update(entity.entity_kind.to_string().as_bytes());
            hasher.update(b":");
            hasher.update(entity.value.as_bytes());
            let hash = hasher.finalize();
            let id: u32 = u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]);
            format!("{}_{}", entity.entity_kind, id)
        }

        TextStrategy::Aggregate => {
            // Numeric aggregation: round to nearest bucket.
            if let Ok(n) = entity.value.parse::<f64>() {
                let bucket_size = if n.abs() < 10.0 {
                    5.0
                } else if n.abs() < 100.0 {
                    10.0
                } else {
                    100.0
                };
                let lower = (n / bucket_size).floor() * bucket_size;
                format!("{:.0}-{:.0}", lower, lower + bucket_size)
            } else {
                format!("[AGG:{}]", entity.entity_kind)
            }
        }

        TextStrategy::Generalize { level } => {
            // Truncation-based generalization: mask the last N characters.
            let lvl = level.unwrap_or(1) as usize;
            let chars: Vec<char> = entity.value.chars().collect();
            if chars.len() > lvl {
                let visible = chars.len() - lvl;
                let prefix: String = chars[..visible].iter().collect();
                let masked: String = std::iter::repeat_n('*', lvl).collect();
                format!("{prefix}{masked}")
            } else {
                "*".repeat(chars.len())
            }
        }

        // Strategies that need external services: produce a tagged
        // placeholder until the service is wired in.
        TextStrategy::Encrypt { .. } => format!("[ENC:{}]", entity.entity_kind),
        TextStrategy::Generate => format!("[GEN:{}]", entity.entity_kind),
        TextStrategy::Tokenize { .. } => format!("[TOKEN:{}]", entity.entity_kind),
        _ => format!("[REDACTED:{}]", entity.entity_kind),
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{Entity, EntityCategory, EntityKind, RecognitionMethod};

    use super::*;

    fn test_entity(value: &str) -> Entity {
        Entity::builder()
            .with_category(EntityCategory::PersonalIdentity)
            .with_entity_kind(EntityKind::PersonName)
            .with_value(value)
            .with_recognition_methods(vec![RecognitionMethod::regex("test")])
            .with_confidence(0.9)
            .build()
            .unwrap()
    }

    #[test]
    fn mask_replaces_with_char() {
        let entity = test_entity("John Smith");
        let result = build_text_replacement(&entity, &TextStrategy::Mask { mask_char: '*' });
        assert_eq!(result, "**********");
    }

    #[test]
    fn mask_custom_char() {
        let entity = test_entity("secret");
        let result = build_text_replacement(&entity, &TextStrategy::Mask { mask_char: 'X' });
        assert_eq!(result, "XXXXXX");
    }

    #[test]
    fn replace_default_placeholder() {
        let entity = test_entity("John");
        let result = build_text_replacement(
            &entity,
            &TextStrategy::Replace {
                placeholder: String::new(),
            },
        );
        assert_eq!(result, "[PERSON_NAME]");
    }

    #[test]
    fn replace_template_no_value_leak() {
        let entity = test_entity("secret");
        let result = build_text_replacement(
            &entity,
            &TextStrategy::Replace {
                placeholder: "{entityType} was here {value}".into(),
            },
        );
        assert!(result.contains("{value}"));
        assert!(!result.contains("secret"));
    }

    #[test]
    fn remove_produces_empty() {
        let entity = test_entity("anything");
        let result = build_text_replacement(&entity, &TextStrategy::Remove);
        assert!(result.is_empty());
    }

    #[test]
    fn hash_is_deterministic() {
        let entity = test_entity("John Smith");
        let r1 = build_text_replacement(&entity, &TextStrategy::Hash);
        let r2 = build_text_replacement(&entity, &TextStrategy::Hash);
        assert_eq!(r1, r2);
        assert!(!r1.contains("John"));
        assert_eq!(r1.len(), 16);
    }

    #[test]
    fn pseudonymize_is_deterministic() {
        let entity = test_entity("John Smith");
        let r1 = build_text_replacement(&entity, &TextStrategy::Pseudonymize);
        let r2 = build_text_replacement(&entity, &TextStrategy::Pseudonymize);
        assert_eq!(r1, r2);
        assert!(!r1.contains("John"));
        assert!(r1.starts_with("person_name_"));
    }

    #[test]
    fn pseudonymize_different_values_differ() {
        let e1 = test_entity("John Smith");
        let e2 = test_entity("Jane Doe");
        let r1 = build_text_replacement(&e1, &TextStrategy::Pseudonymize);
        let r2 = build_text_replacement(&e2, &TextStrategy::Pseudonymize);
        assert_ne!(r1, r2);
    }

    #[test]
    fn aggregate_numeric() {
        let entity = test_entity("34");
        let result = build_text_replacement(&entity, &TextStrategy::Aggregate);
        assert_eq!(result, "30-40");
    }

    #[test]
    fn aggregate_non_numeric_falls_back() {
        let entity = test_entity("not a number");
        let result = build_text_replacement(&entity, &TextStrategy::Aggregate);
        assert!(result.starts_with("[AGG:"));
    }

    #[test]
    fn generalize_truncates() {
        let entity = test_entity("94107");
        let result =
            build_text_replacement(&entity, &TextStrategy::Generalize { level: Some(2) });
        assert_eq!(result, "941**");
    }

    #[test]
    fn generalize_default_level() {
        let entity = test_entity("94107");
        let result = build_text_replacement(&entity, &TextStrategy::Generalize { level: None });
        assert_eq!(result, "9410*");
    }

    #[test]
    fn generalize_short_value() {
        let entity = test_entity("AB");
        let result =
            build_text_replacement(&entity, &TextStrategy::Generalize { level: Some(5) });
        assert_eq!(result, "**");
    }
}

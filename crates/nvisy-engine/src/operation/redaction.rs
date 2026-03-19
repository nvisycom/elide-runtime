//! Redaction: policy evaluation + content redaction.
//!
//! Evaluates policy rules against detected entities to produce redaction
//! decisions, then (when content-level redaction is wired) applies those
//! decisions to the document content.

use nvisy_core::{Error, Result};
use nvisy_ontology::entity::{Entities, Entity};
use nvisy_ontology::policy::{PolicyRule, RuleAction, Strategy, TextStrategy};

use crate::operation::envelope::PolicyOutcome;
use crate::operation::{DocumentEnvelope, NodeHandler, Operation, ParallelContext, SharedContext};
use crate::pipeline::CompiledRetryPolicy;
use crate::provenance::{RedactionDecision, RedactionRecord};

const TARGET: &str = "nvisy_engine::op::redaction";

/// Redaction operation: evaluates policies and applies redaction decisions.
pub struct Redaction {
    evaluator: PolicyEvaluator,
    shared: SharedContext,
    retry: Option<CompiledRetryPolicy>,
}

impl Redaction {
    /// Build from graph config and shared context.
    pub async fn connect(
        cfg: &crate::graph::Redaction,
        shared: SharedContext,
        retry: Option<CompiledRetryPolicy>,
    ) -> Result<Self> {
        let rules = shared
            .policies
            .policies
            .iter()
            .flat_map(|p| p.rules.clone())
            .collect();

        let evaluator = PolicyEvaluator::new(rules);

        if cfg.process_metadata {
            tracing::debug!(target: TARGET, "metadata processing handled by policy evaluation");
        }

        Ok(Self {
            evaluator,
            shared,
            retry,
        })
    }
}

#[async_trait::async_trait]
impl NodeHandler for Redaction {
    async fn handle(&self, mut envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        if !envelope.entities.is_empty() {
            let eval_ref = &self.evaluator;
            let retry = self.retry.as_ref();
            let do_eval = || {
                let entities = envelope.entities.clone();
                let shared = self.shared.clone();
                async move {
                    let input = ParallelContext::new(entities, shared);
                    eval_ref.call(input).await
                }
            };
            let output = match retry {
                Some(policy) => policy.with_retry(do_eval).await?,
                None => do_eval().await?,
            };
            envelope.apply(output.into_inner());
        }

        // Content-level redaction (byte-level text replacement, image blur,
        // audio silence) requires handler downcast from the type-erased
        // Document. Policy evaluation above populated the audit decisions
        // and records. Content mutation will be wired when the codec
        // exposes a modality-agnostic redaction API.

        Ok(envelope)
    }
}

// ── Internal: Policy Evaluator ────────────────────────────────────

struct PolicyEvaluator {
    rules: Vec<PolicyRule>,
    default_spec: Strategy,
    default_threshold: f64,
}

impl PolicyEvaluator {
    fn new(mut rules: Vec<PolicyRule>) -> Self {
        rules.sort_by_key(|r| r.priority);
        Self {
            rules,
            default_spec: Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            default_threshold: 0.5,
        }
    }

    async fn evaluate(&self, entities: Entities) -> Result<PolicyOutcome> {
        tracing::debug!(target: TARGET, entity_count = entities.len(), "evaluating policies");
        let mut decisions = Vec::new();
        let mut records = Vec::new();

        for entity in &entities {
            let rule = self.find_matching_rule(entity);

            let (spec, replacement) = match rule {
                Some(r) => match &r.action {
                    RuleAction::Redact { strategy } => {
                        (strategy.clone(), Self::build_replacement(entity, strategy))
                    }
                    action @ (RuleAction::Review
                    | RuleAction::Alert
                    | RuleAction::Block
                    | RuleAction::Suppress) => {
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
                        Self::build_replacement(entity, &self.default_spec),
                    )
                }
            };

            let entity_id = entity.source.as_uuid();

            let mut decision =
                RedactionDecision::new(entity_id, spec, replacement, entity.confidence);
            if let Some(r) = rule {
                decision = decision.with_policy_rule_id(r.id);
            }
            decision.source.set_parent_id(Some(entity_id));

            let mut record = RedactionRecord::new(entity_id, &entity.value, entity.confidence);
            if let Some(r) = rule {
                record = record.with_policy_rule_id(r.id);
            }
            record.source.set_parent_id(Some(entity_id));

            decisions.push(decision);
            records.push(record);
        }

        Ok(PolicyOutcome { decisions, records })
    }

    fn find_matching_rule(&self, entity: &Entity) -> Option<&PolicyRule> {
        self.rules.iter().find(|rule| {
            rule.selector
                .matches(&entity.category, entity.entity_kind, entity.confidence)
        })
    }

    fn build_replacement(entity: &Entity, spec: &Strategy) -> String {
        match spec {
            Strategy::Text(text) => match text {
                TextStrategy::Mask { mask_char } => {
                    mask_char.to_string().repeat(entity.value.len())
                }
                TextStrategy::Replace { placeholder } => {
                    if placeholder.is_empty() {
                        format!("[{}]", entity.entity_kind.to_string().to_uppercase())
                    } else {
                        placeholder
                            .replace("{entityType}", &entity.entity_kind.to_string())
                            .replace("{category}", &entity.category.to_string())
                            .replace("{value}", &entity.value)
                    }
                }
                TextStrategy::Remove => String::new(),
                TextStrategy::Hash => format!("[HASH:{}]", entity.entity_kind),
                TextStrategy::Encrypt { .. } => format!("[ENC:{}]", entity.entity_kind),
                TextStrategy::Generate => format!("[GEN:{}]", entity.entity_kind),
                TextStrategy::Pseudonymize => format!("[PSEUDO:{}]", entity.entity_kind),
                TextStrategy::Tokenize { .. } => format!("[TOKEN:{}]", entity.entity_kind),
                TextStrategy::Aggregate => format!("[AGG:{}]", entity.entity_kind),
                TextStrategy::Generalize { .. } => {
                    format!("[GENERALIZE:{}]", entity.entity_kind)
                }
            },
            Strategy::Image(_) | Strategy::Audio(_) => String::new(),
        }
    }
}

impl Operation for PolicyEvaluator {
    type Input = ParallelContext<Entities>;
    type Output = ParallelContext<PolicyOutcome>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.evaluate(data)).await
    }
}

//! Redaction operation.
//!
//! Runs at **phase 4** alongside [`GenerateContextOp`]. Evaluates policy
//! rules against detected entities to produce redaction decisions and
//! replacement text.
//!
//! [`GenerateContextOp`]: crate::operation::GenerateContextOp

use std::collections::HashMap;
use std::sync::Mutex;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use nvisy_core::{Error, Result};
use nvisy_ontology::entity::{Entities, Entity, EntityKind};
use nvisy_ontology::policy::{PolicyRule, RuleAction, Strategy, TextStrategy};
use nvisy_ontology::provenance::{RedactionDecision, RedactionRecord};
use nvisy_ontology::workflow::Redaction;
use nvisy_provider::agent::{AgentConfig, AgentProvider, GenAgent, GenRequest};
use nvisy_provider::http::HttpClient;
use sha2::{Digest, Sha256};

use crate::operation::Operation;
use crate::operation::context::{ParallelContext, SharedContext};
use crate::operation::encryption::SharedKeyProvider;
use crate::operation::envelope::PolicyOutcome;
use crate::pipeline::RuntimeConfig;

const TARGET: &str = "nvisy_engine::op::redaction";

/// Redaction operation: evaluates policies and produces redaction decisions.
pub struct RedactionOp {
    evaluator: PolicyEvaluator,
}

impl RedactionOp {
    /// Build from graph config, shared context, and runtime config.
    pub async fn new(
        cfg: &Redaction,
        shared: &SharedContext,
        config: &RuntimeConfig,
        http_client: &HttpClient,
    ) -> Result<Self> {
        let mut rules: Vec<PolicyRule> = shared
            .policies
            .policies
            .iter()
            .flat_map(|p| p.rules.clone())
            .collect();
        rules.sort_by_key(|r| r.priority);

        let gen_agent = build_gen_agent(config, http_client);

        let evaluator = PolicyEvaluator {
            rules,
            default_spec: Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            default_threshold: cfg.confidence_threshold.unwrap_or(0.5),
            key_provider: shared.key_provider.clone(),
            gen_agent,
            token_vault: Mutex::new(HashMap::new()),
        };

        Ok(Self { evaluator })
    }
}

fn build_gen_agent(config: &RuntimeConfig, http_client: &HttpClient) -> Option<GenAgent> {
    let llm = config.llm.as_ref()?;
    let provider = llm.provider.clone()?;
    let agent_config = llm.policy.clone().unwrap_or_default();
    GenAgent::new(&provider, agent_config, Some(http_client.clone())).ok()
}

impl Operation for RedactionOp {
    type Input = ParallelContext<Entities>;
    type Output = ParallelContext<PolicyOutcome>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input
            .parallel_map(|data| self.evaluator.evaluate(data))
            .await
    }
}

struct PolicyEvaluator {
    rules: Vec<PolicyRule>,
    default_spec: Strategy,
    default_threshold: f64,
    key_provider: SharedKeyProvider,
    gen_agent: Option<GenAgent>,
    token_vault: Mutex<HashMap<String, String>>,
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
                        let replacement = self.build_replacement(entity, strategy).await;
                        (strategy.clone(), replacement)
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
                    let replacement = self.build_replacement(entity, &self.default_spec).await;
                    (self.default_spec.clone(), replacement)
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

    async fn build_replacement(&self, entity: &Entity, spec: &Strategy) -> String {
        match spec {
            Strategy::Text(text) => self.build_text_replacement(entity, text).await,
            Strategy::Image(_) | Strategy::Audio(_) | _ => String::new(),
        }
    }

    async fn build_text_replacement(&self, entity: &Entity, strategy: &TextStrategy) -> String {
        match strategy {
            TextStrategy::Mask { mask_char } => mask_char.to_string().repeat(entity.value.len()),

            TextStrategy::Replace { placeholder } => {
                if placeholder.is_empty() {
                    format!("[{}]", entity.entity_kind.to_string().to_uppercase())
                } else {
                    placeholder
                        .replace("{entityType}", &entity.entity_kind.to_string())
                        .replace("{category}", &entity.category.to_string())
                }
            }

            TextStrategy::Remove => String::new(),

            TextStrategy::Hash => hash_value(&entity.value),

            TextStrategy::Encrypt { key_id } => {
                encrypt_value(&entity.value, key_id, &self.key_provider)
            }

            TextStrategy::Generate => {
                self.generate_value(entity.entity_kind, &entity.value).await
            }

            TextStrategy::Pseudonymize => pseudonymize(&entity.entity_kind, &entity.value),

            TextStrategy::Tokenize { vault_id } => {
                self.tokenize(&entity.value, vault_id.as_deref())
            }

            TextStrategy::Aggregate => aggregate(&entity.value, entity.entity_kind),

            TextStrategy::Generalize { level } => generalize(&entity.value, *level),

            _ => format!("[REDACTED:{}]", entity.entity_kind),
        }
    }

    async fn generate_value(&self, entity_type: EntityKind, original: &str) -> String {
        if let Some(agent) = &self.gen_agent {
            let request = GenRequest {
                entity_type,
                original_value: original.to_owned(),
                context: None,
            };
            match agent.generate_one(&request).await {
                Ok(generated) => return generated.synthetic_value,
                Err(e) => {
                    tracing::warn!(
                        target: TARGET,
                        error = %e,
                        "generate failed, falling back to pseudonym",
                    );
                }
            }
        }
        pseudonymize(&entity_type, original)
    }

    fn tokenize(&self, value: &str, vault_id: Option<&str>) -> String {
        let mut vault = self.token_vault.lock().expect("token vault lock poisoned");
        if let Some(token) = vault.get(value) {
            return token.clone();
        }
        let prefix = vault_id.unwrap_or("tok");
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        hasher.update(prefix.as_bytes());
        let hash = hasher.finalize();
        let id: u64 = u64::from_le_bytes([
            hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7],
        ]);
        let token = format!("{prefix}_{id:016x}");
        vault.insert(value.to_owned(), token.clone());
        token
    }
}

fn hash_value(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let hash = hasher.finalize();
    hash.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

fn encrypt_value(value: &str, key_id: &str, key_provider: &SharedKeyProvider) -> String {
    use crate::operation::encryption::KeyProvider;

    let raw_key = match key_provider.resolve(key_id) {
        Ok(k) => k,
        Err(e) => {
            tracing::warn!(target: TARGET, key_id, error = %e, "encrypt key not found, falling back to hash");
            return hash_value(value);
        }
    };

    let cipher = match Aes256Gcm::new_from_slice(&raw_key) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(target: TARGET, error = %e, "invalid AES key, falling back to hash");
            return hash_value(value);
        }
    };

    // Deterministic nonce for consistent output on the same value.
    let nonce_bytes = [0u8; 12];
    let nonce = Nonce::from_slice(&nonce_bytes);

    match cipher.encrypt(nonce, value.as_bytes()) {
        Ok(ciphertext) => BASE64.encode(&ciphertext),
        Err(e) => {
            tracing::warn!(target: TARGET, error = %e, "encryption failed, falling back to hash");
            hash_value(value)
        }
    }
}

fn pseudonymize(entity_kind: &EntityKind, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(entity_kind.to_string().as_bytes());
    hasher.update(b":");
    hasher.update(value.as_bytes());
    let hash = hasher.finalize();
    let id: u32 = u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]);
    format!("{entity_kind}_{id}")
}

fn aggregate(value: &str, entity_kind: EntityKind) -> String {
    if let Ok(n) = value.parse::<f64>() {
        let bucket_size = if n.abs() < 10.0 {
            5.0
        } else if n.abs() < 100.0 {
            10.0
        } else {
            100.0
        };
        let lower = (n / bucket_size).floor() * bucket_size;
        format!("{lower:.0}-{:.0}", lower + bucket_size)
    } else {
        format!("[AGG:{entity_kind}]")
    }
}

fn generalize(value: &str, level: Option<u32>) -> String {
    let lvl = level.unwrap_or(1) as usize;
    let chars: Vec<char> = value.chars().collect();
    if chars.len() > lvl {
        let visible = chars.len() - lvl;
        let prefix: String = chars[..visible].iter().collect();
        let masked: String = std::iter::repeat_n('*', lvl).collect();
        format!("{prefix}{masked}")
    } else {
        "*".repeat(chars.len())
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
    fn hash_is_deterministic() {
        let r1 = hash_value("John Smith");
        let r2 = hash_value("John Smith");
        assert_eq!(r1, r2);
        assert!(!r1.contains("John"));
        assert_eq!(r1.len(), 16);
    }

    #[test]
    fn hash_different_values_differ() {
        assert_ne!(hash_value("John"), hash_value("Jane"));
    }

    #[test]
    fn pseudonymize_is_deterministic() {
        let r1 = pseudonymize(&EntityKind::PersonName, "John Smith");
        let r2 = pseudonymize(&EntityKind::PersonName, "John Smith");
        assert_eq!(r1, r2);
        assert!(!r1.contains("John"));
        assert!(r1.starts_with("person_name_"));
    }

    #[test]
    fn pseudonymize_different_values_differ() {
        let r1 = pseudonymize(&EntityKind::PersonName, "John Smith");
        let r2 = pseudonymize(&EntityKind::PersonName, "Jane Doe");
        assert_ne!(r1, r2);
    }

    #[test]
    fn aggregate_numeric() {
        assert_eq!(aggregate("34", EntityKind::PersonName), "30-40");
        assert_eq!(aggregate("7", EntityKind::PersonName), "5-10");
        assert_eq!(aggregate("150", EntityKind::PersonName), "100-200");
    }

    #[test]
    fn aggregate_non_numeric() {
        let result = aggregate("not a number", EntityKind::PersonName);
        assert!(result.starts_with("[AGG:"));
    }

    #[test]
    fn generalize_truncates() {
        assert_eq!(generalize("94107", Some(2)), "941**");
        assert_eq!(generalize("94107", None), "9410*");
        assert_eq!(generalize("AB", Some(5)), "**");
    }

    #[test]
    fn encrypt_falls_back_to_hash_on_missing_key() {
        let result = encrypt_value("secret", "nonexistent", &SharedKeyProvider::default());
        assert_eq!(result, hash_value("secret"));
    }

    #[test]
    fn tokenize_is_deterministic() {
        let evaluator = PolicyEvaluator {
            rules: Vec::new(),
            default_spec: Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            default_threshold: 0.5,
            key_provider: SharedKeyProvider::default(),
            gen_agent: None,
            token_vault: Mutex::new(HashMap::new()),
        };
        let t1 = evaluator.tokenize("secret", None);
        let t2 = evaluator.tokenize("secret", None);
        assert_eq!(t1, t2);
        assert!(t1.starts_with("tok_"));
    }

    #[test]
    fn tokenize_different_values_differ() {
        let evaluator = PolicyEvaluator {
            rules: Vec::new(),
            default_spec: Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            default_threshold: 0.5,
            key_provider: SharedKeyProvider::default(),
            gen_agent: None,
            token_vault: Mutex::new(HashMap::new()),
        };
        assert_ne!(
            evaluator.tokenize("a", None),
            evaluator.tokenize("b", None)
        );
    }

    #[test]
    fn tokenize_with_vault_id() {
        let evaluator = PolicyEvaluator {
            rules: Vec::new(),
            default_spec: Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            default_threshold: 0.5,
            key_provider: SharedKeyProvider::default(),
            gen_agent: None,
            token_vault: Mutex::new(HashMap::new()),
        };
        let token = evaluator.tokenize("secret", Some("vault"));
        assert!(token.starts_with("vault_"));
    }
}

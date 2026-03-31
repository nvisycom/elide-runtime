//! Post-redaction validation operation.
//!
//! Runs at **phase 5**, after [`Redaction`]. Re-scans redacted content
//! to verify that no originally detected values remain visible.
//!
//! [`Redaction`]: crate::operation::RedactionOp

use nvisy_core::{Error, Result};
use nvisy_ontology::entity::Entities;
use nvisy_ontology::provenance::RedactionDecision;
use nvisy_ontology::workflow::Validation;
use uuid::Uuid;

use crate::operation::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::validation";

/// A sensitive value that was not properly redacted.
#[derive(Debug, Clone)]
pub struct LeakedValue {
    pub value: String,
    pub entity_id: Uuid,
}

/// Result of validation.
pub struct ValidationResult {
    pub passed: usize,
    pub leaked: Vec<LeakedValue>,
}

/// Post-redaction validator that checks for leaked sensitive values.
pub struct ValidationOp {
    fail_on_leak: bool,
}

impl ValidationOp {
    /// Create from graph config.
    pub fn new(cfg: &Validation) -> Self {
        Self {
            fail_on_leak: cfg.fail_on_leak,
        }
    }

    fn check(
        entities: &Entities,
        decisions: &[RedactionDecision],
        redacted_text: Option<&str>,
    ) -> ValidationResult {
        let mut passed = 0usize;
        let mut leaked = Vec::new();

        let applied: Vec<_> = decisions.iter().filter(|d| d.applied).collect();

        if let Some(text) = redacted_text {
            let lower_text = text.to_lowercase();
            for decision in &applied {
                let entity = entities
                    .iter()
                    .find(|e| e.source.as_uuid() == decision.entity_id);

                if let Some(entity) = entity {
                    let lower_value = entity.value.to_lowercase();
                    if !entity.value.is_empty() && lower_text.contains(&lower_value) {
                        leaked.push(LeakedValue {
                            value: entity.value.clone(),
                            entity_id: decision.entity_id,
                        });
                    } else {
                        passed += 1;
                    }
                } else {
                    passed += 1;
                }
            }
        } else {
            passed = applied.len();
        }

        ValidationResult { passed, leaked }
    }
}

impl Operation for ValidationOp {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        tracing::debug!(target: TARGET, "running post-redaction validation");

        let text_spans: Vec<_> = envelope.document.collect_text_spans().await;
        let redacted_text = if text_spans.is_empty() {
            None
        } else {
            Some(
                text_spans
                    .iter()
                    .map(|s| s.data.as_str())
                    .collect::<String>(),
            )
        };

        let result = Self::check(
            &envelope.entities,
            &envelope.audit.decisions,
            redacted_text.as_deref(),
        );

        if result.leaked.is_empty() {
            tracing::debug!(target: TARGET, passed = result.passed, "validation passed");
        } else {
            tracing::warn!(
                target: TARGET,
                leaked = result.leaked.len(),
                passed = result.passed,
                "validation found leaked values",
            );
            if self.fail_on_leak {
                let details: Vec<String> = result
                    .leaked
                    .iter()
                    .map(|l| format!("{}({})", l.value, l.entity_id))
                    .collect();
                return Err(Error::validation(
                    format!(
                        "{} redacted value(s) leaked in output: {}",
                        result.leaked.len(),
                        details.join(", "),
                    ),
                    "validation",
                ));
            }
        }

        Ok(())
    }
}

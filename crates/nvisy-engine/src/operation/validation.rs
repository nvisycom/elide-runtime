//! Post-redaction validation operation.

//!
//! Runs at **phase 5**, after [`Redaction`]. Re-scans redacted content
//! to verify that no originally detected values remain visible.
//!
//! [`Redaction`]: crate::operation::Redaction
use nvisy_core::{Error, Result};
use nvisy_ontology::entity::Entities;
use uuid::Uuid;

use crate::operation::Operation;
use crate::operation::context::ParallelContext;
use crate::provenance::RedactionDecision;

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

/// Input for the typed [`Operation`] impl.
#[derive(Clone)]
pub struct ValidationInput {
    /// Entities that were detected.
    pub entities: Entities,
    /// Redaction decisions that should have been applied.
    pub decisions: Vec<RedactionDecision>,
    /// The redacted document content as text (for text-based checks).
    pub redacted_text: Option<String>,
}

/// Post-redaction validator that checks for leaked sensitive values.
pub struct Validation {
    fail_on_leak: bool,
}

impl Validation {
    /// Create from graph config.
    pub fn new(cfg: &crate::graph::Validation) -> Self {
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

impl Operation for Validation {
    type Input = ParallelContext<ValidationInput>;
    type Output = ParallelContext<ValidationResult>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        tracing::debug!(target: TARGET, "running post-redaction validation");
        let fail_on_leak = self.fail_on_leak;
        input
            .parallel_map(|data| async move {
                let result = Self::check(
                    &data.entities,
                    &data.decisions,
                    data.redacted_text.as_deref(),
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
                    if fail_on_leak {
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
                Ok(result)
            })
            .await
    }
}

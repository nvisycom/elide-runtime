//! Post-redaction validation operation.
//!
//! Runs at **phase 5**, after [`Redaction`]. Re-scans redacted content
//! to verify that no originally detected values remain visible.
//!
//! [`Redaction`]: crate::operation::RedactionOp

use nvisy_core::{Error, Result};
use nvisy_ontology::entity::Entities;
use nvisy_ontology::provenance::AuditEntry;
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

    /// Check whether any redacted values leaked in the output text.
    ///
    /// Only entities with a text value (resolved from the document)
    /// can be verified this way. Image/audio entities without text
    /// values are counted as passed — visual and temporal redaction
    /// verification is not yet implemented.
    async fn check(
        entities: &Entities,
        records: &[AuditEntry],
        redacted_text: Option<&str>,
        document: &crate::operation::Document,
    ) -> ValidationResult {
        let mut passed = 0usize;
        let mut leaked = Vec::new();

        let applied: Vec<_> = records.iter().filter(|r| r.redaction.is_applied).collect();

        if let Some(text) = redacted_text {
            let lower_text = text.to_lowercase();
            for record in &applied {
                let entity = entities.iter().find(|e| e.id == record.entity_id);

                if let Some(entity) = entity {
                    if let Some(value) = document.value_at(&entity.location).await {
                        let lower_value = value.to_lowercase();
                        if !value.is_empty() && lower_text.contains(&lower_value) {
                            leaked.push(LeakedValue {
                                value,
                                entity_id: record.entity_id,
                            });
                        } else {
                            passed += 1;
                        }
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

        let locations = envelope.document.collect_text_locations().await;
        let redacted_text = if locations.is_empty() {
            None
        } else {
            let mut buf = String::new();
            for located in &locations {
                if let Some(data) = envelope.document.read_text(&located.location).await {
                    buf.push_str(data.as_str());
                }
            }
            Some(buf)
        };

        let result = Self::check(
            &envelope.audit.entities,
            &envelope.audit.entries,
            redacted_text.as_deref(),
            &envelope.document,
        )
        .await;

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

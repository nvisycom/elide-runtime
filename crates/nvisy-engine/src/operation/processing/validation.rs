//! Post-redaction validation.
//!
//! Checks that redacted content does not contain any of the original
//! sensitive values that were supposed to be redacted.

use nvisy_core::Result;
use nvisy_ontology::entity::Entities;

use crate::operation::{Operation, ParallelContext};
use crate::provenance::RedactionDecision;

const TARGET: &str = "nvisy_engine::op::validation";

/// Input for the validation operation.
#[derive(Clone)]
pub struct ValidationInput {
    /// Entities that were detected.
    pub entities: Entities,
    /// Redaction decisions that should have been applied.
    pub decisions: Vec<RedactionDecision>,
    /// The redacted document content as text (for text-based checks).
    pub redacted_text: Option<String>,
}

/// Result of validation: which redactions passed and which leaked.
pub struct ValidationOutput {
    /// Number of redactions that passed validation.
    pub passed: usize,
    /// Values that were supposed to be redacted but still appear in the output.
    pub leaked: Vec<LeakedValue>,
}

/// A sensitive value that was not properly redacted.
#[derive(Debug, Clone)]
pub struct LeakedValue {
    /// The original sensitive value.
    pub value: String,
    /// Entity ID this value belongs to.
    pub entity_id: uuid::Uuid,
}

/// Validates that applied redactions actually removed sensitive content.
pub struct Validation;

impl Validation {
    async fn validate(&self, input: ValidationInput) -> Result<ValidationOutput> {
        let mut passed = 0usize;
        let mut leaked = Vec::new();

        let applied: Vec<_> = input.decisions.iter().filter(|d| d.applied).collect();

        if let Some(ref text) = input.redacted_text {
            for decision in &applied {
                let entity = input
                    .entities
                    .iter()
                    .find(|e| e.source.as_uuid() == decision.entity_id);

                if let Some(entity) = entity {
                    let lower_text = text.to_lowercase();
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

        tracing::debug!(
            target: TARGET,
            passed,
            leaked = leaked.len(),
            total = applied.len(),
            "validation complete",
        );

        Ok(ValidationOutput { passed, leaked })
    }
}

impl Operation for Validation {
    type Input = ParallelContext<ValidationInput>;
    type Output = ParallelContext<ValidationOutput>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.validate(data)).await
    }
}

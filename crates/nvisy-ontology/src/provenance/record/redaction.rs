//! Redaction record: unified decision + audit trail for a single entity.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::review::ReviewDecision;
use crate::entity::{ContentSource, Location};
use crate::policy::Strategy;

/// A complete redaction record for a single entity: what strategy was
/// chosen, where it applies, what the original and replacement values
/// are, and the review lifecycle.
///
/// Created by the policy evaluator via the builder, then enriched by
/// the applicator with the replacement value and `is_applied` flag.
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "RedactionRecordBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct RedactionRecord {
    /// Content source identity and lineage.
    #[builder(default)]
    pub source: ContentSource,
    /// Identifier of the entity being redacted.
    pub entity_id: Uuid,
    /// Identifier of the policy that triggered this redaction.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<Uuid>,
    /// What to do and where: strategy, location, and application state.
    pub redaction: RedactionSpec,
    /// Original and replacement values with detection confidence.
    pub value: RedactionValue,
    /// Versioning and human review state.
    #[builder(default)]
    pub lifecycle: RedactionLifecycle,
}

/// Strategy, location, and application state for a redaction.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionSpec {
    /// Redaction strategy to apply.
    pub strategy: Strategy,
    /// Modality-specific location of the entity being redacted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    /// Whether the redaction has been applied to the output content.
    pub is_applied: bool,
    /// Whether the original can be reconstructed from this redaction.
    pub reversible: bool,
}

/// Original and replacement values with detection confidence.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionValue {
    /// The original sensitive value.
    pub original: String,
    /// The replacement value after redaction was applied.
    ///
    /// `None` until the redaction is applied, or when the strategy
    /// removes the value entirely.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
    /// Detection confidence that led to this redaction.
    pub confidence: f64,
}

/// Versioning and human review lifecycle.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionLifecycle {
    /// Version of this record (starts at 1, incremented on modification).
    #[serde(default = "default_version")]
    pub version: u32,
    /// Human review decision, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewDecision>,
}

fn default_version() -> u32 {
    1
}

impl RedactionRecord {
    /// Start building a new redaction record.
    pub fn builder() -> RedactionRecordBuilder {
        RedactionRecordBuilder::default()
    }

    /// The unique identifier for this record.
    pub fn id(&self) -> Uuid {
        self.source.as_uuid()
    }
}

impl RedactionRecordBuilder {
    /// Set the entity ID, strategy, original value, and confidence in one call.
    ///
    /// This is the most common construction pattern: the policy evaluator
    /// knows the entity, the strategy it matched, and the original value.
    pub fn for_entity(
        self,
        entity_id: Uuid,
        strategy: Strategy,
        original: impl Into<String>,
        confidence: f64,
    ) -> Self {
        self.with_entity_id(entity_id)
            .with_redaction(RedactionSpec {
                strategy,
                location: None,
                is_applied: false,
                reversible: false,
            })
            .with_value(RedactionValue {
                original: original.into(),
                replacement: None,
                confidence,
            })
    }
}

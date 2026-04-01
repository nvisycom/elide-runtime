//! Audit entry: per-entity redaction record with provenance metadata.

use derive_builder::Builder;
use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

use super::review::ReviewDecision;
use crate::entity::{ContentSource, Location};
use crate::policy::Strategy;

/// Outcome status of a redaction operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum AuditEntryStatus {
    /// Redaction completed successfully.
    Success,
    /// Redaction failed.
    Failed,
    /// Redaction completed with partial results.
    Partial,
    /// Redaction is pending (not yet applied).
    Pending,
}

/// A per-entity audit entry: what strategy was chosen, where it applies,
/// what the original and replacement values are, and optional review.
///
/// Created by the policy evaluator via the builder, then enriched by
/// the applicator with the replacement value and `is_applied` flag.
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "AuditEntryBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    /// Content source identity and lineage.
    #[builder(default)]
    pub source: ContentSource,
    /// Identifier of the entity being redacted.
    pub entity_id: Uuid,
    /// Identifier of the policy that triggered this redaction.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<Uuid>,
    /// When this entry was created.
    #[builder(default = "Timestamp::now()")]
    #[schemars(with = "String")]
    pub timestamp: Timestamp,
    /// Outcome status of the redaction.
    #[builder(default = "AuditEntryStatus::Pending")]
    pub status: AuditEntryStatus,
    /// Correlation identifier for tracing across services.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Uuid>,
    /// What to do and where: strategy, location, and application state.
    pub redaction: RedactionSpec,
    /// Original and replacement values with detection confidence.
    pub value: RedactionValue,
    /// Human review decision, if any.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewDecision>,
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

impl AuditEntry {
    /// Start building a new audit entry.
    pub fn builder() -> AuditEntryBuilder {
        AuditEntryBuilder::default()
    }

    /// The unique identifier for this entry.
    pub fn id(&self) -> Uuid {
        self.source.as_uuid()
    }
}

impl AuditEntryBuilder {
    /// Set the entity ID, strategy, original value, and confidence in one call.
    ///
    /// This is the most common construction pattern: the policy evaluator
    /// knows the entity, the strategy it matched, and the original value.
    /// `reversible` is derived from the strategy, and `location` defaults
    /// to `None` (set separately via [`with_location`](Self::with_location)).
    pub fn for_entity(
        self,
        entity_id: Uuid,
        strategy: Strategy,
        original: impl Into<String>,
        confidence: f64,
    ) -> Self {
        let reversible = strategy.is_reversible();
        self.with_entity_id(entity_id)
            .with_redaction(RedactionSpec {
                strategy,
                location: None,
                is_applied: false,
                reversible,
            })
            .with_value(RedactionValue {
                original: original.into(),
                replacement: None,
                confidence,
            })
    }

    /// Set the modality-specific location on the redaction spec.
    ///
    /// Must be called after [`for_entity`](Self::for_entity).
    pub fn with_location(mut self, location: Location) -> Self {
        if let Some(ref mut spec) = self.redaction {
            spec.location = Some(location);
        }
        self
    }

    /// Set the parent content source for lineage tracking.
    pub fn with_parent_id(mut self, parent_id: Uuid) -> Self {
        if let Some(ref mut source) = self.source {
            source.set_parent_id(Some(parent_id));
        }
        self
    }
}

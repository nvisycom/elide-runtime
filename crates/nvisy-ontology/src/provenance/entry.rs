//! Audit entry: per-entity redaction record with provenance metadata.

use derive_builder::Builder;
use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

use super::review::ReviewDecision;
use crate::modality::AnyModality;
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
    /// Entity was deliberately not redacted because a matching
    /// [`Action::Suppress`] rule won out over any matching Redact rule
    /// (or no Redact rule matched). The entry records the suppressing
    /// policy and the original value so the suppression is auditable.
    ///
    /// [`Action::Suppress`]: crate::policy::Action::Suppress
    Suppressed,
}

/// A per-entity audit entry: what strategy was chosen, what the
/// original and replacement values are, and optional review.
///
/// Created by the policy evaluator via the builder, then enriched by
/// the applicator with the replacement value and `is_applied` flag.
///
/// Location and confidence are not stored here: they live on the
/// corresponding [`Entity`] in
/// [`Audit::entities`], linked by `entity_id`.
///
/// [`Entity`]: crate::entity::Entity
/// [`Audit::entities`]: super::Audit::entities
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "AuditEntryBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
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
    /// What to do: strategy and application state.
    pub redaction: RedactionSpec,
    /// Original and replacement values.
    pub value: RedactionValue,
    /// Human review decision, if any.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewDecision>,
}

/// Strategy and application state for a redaction.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionSpec {
    /// Redaction strategy to apply.
    pub strategy: Strategy,
    /// Whether the redaction has been applied to the output content.
    pub is_applied: bool,
    /// Whether the original can be reconstructed from this redaction.
    pub reversible: bool,
}

/// Original and replacement values for a redaction.
///
/// `original` is the portion of the entity value that was redacted,
/// which may differ from the full entity value depending on the policy.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionValue {
    /// The original sensitive value that was redacted.
    pub original: String,
    /// The replacement value after redaction was applied.
    ///
    /// `None` until the redaction is applied, or when the strategy
    /// removes the value entirely.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
}

impl AuditEntry {
    /// Start building a new audit entry.
    pub fn builder() -> AuditEntryBuilder {
        AuditEntryBuilder::default()
    }
}

impl AuditEntryBuilder {
    /// Set the entity ID, strategy, and original value in one call.
    ///
    /// `reversible` is derived from the strategy resolved against
    /// `location`'s modality.
    pub fn for_entity(
        self,
        entity_id: Uuid,
        strategy: Strategy,
        original: impl Into<String>,
        location: &AnyModality,
    ) -> Self {
        let reversible = strategy.is_reversible_for(location);
        self.with_entity_id(entity_id)
            .with_redaction(RedactionSpec {
                strategy,
                is_applied: false,
                reversible,
            })
            .with_value(RedactionValue {
                original: original.into(),
                replacement: None,
            })
    }
}

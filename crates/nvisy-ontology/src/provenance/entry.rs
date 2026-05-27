//! Audit entry: per-entity redaction record with provenance metadata.

use derive_builder::Builder;
use jiff::Timestamp;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

use super::review::ReviewDecision;
use crate::modality::{Modality, RedactionStrategy};

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
    /// [`Action::Suppress`] rule won out over any matching Redact
    /// rule. The entry records the suppressing policy and the
    /// original value so the suppression is auditable.
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
/// corresponding [`Entity<M>`] in [`Audit::entities`], linked by
/// `entity_id`.
///
/// [`Entity<M>`]: crate::entity::Entity
/// [`Audit::entities`]: super::Audit::entities
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "AuditEntryBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(
    rename_all = "camelCase",
    bound(
        serialize = "M::Strategy: Serialize",
        deserialize = "M::Strategy: DeserializeOwned",
    )
)]
#[schemars(bound = "M::Strategy: JsonSchema")]
pub struct AuditEntry<M: Modality> {
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
    pub redaction: RedactionSpec<M>,
    /// Original and replacement values.
    pub value: RedactionValue,
    /// Human review decision, if any.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewDecision>,
}

/// Strategy and application state for a redaction.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(
    rename_all = "camelCase",
    bound(
        serialize = "M::Strategy: Serialize",
        deserialize = "M::Strategy: DeserializeOwned",
    )
)]
#[schemars(bound = "M::Strategy: JsonSchema")]
pub struct RedactionSpec<M: Modality> {
    /// Redaction strategy to apply.
    pub strategy: M::Strategy,
    /// Whether the redaction has been applied to the output content.
    pub is_applied: bool,
    /// Whether the original can be reconstructed from this redaction.
    pub reversible: bool,
}

/// Original and replacement values for a redaction.
///
/// `original` is the portion of the entity value that was redacted,
/// which may differ from the full entity value depending on the
/// policy.
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

impl<M: Modality> AuditEntry<M> {
    /// Start building a new audit entry.
    pub fn builder() -> AuditEntryBuilder<M> {
        AuditEntryBuilder::default()
    }
}

impl<M: Modality> AuditEntryBuilder<M>
where
    M::Strategy: RedactionStrategy,
{
    /// Set the entity ID, strategy, and original value in one call.
    pub fn for_entity(
        self,
        entity_id: Uuid,
        strategy: M::Strategy,
        original: impl Into<String>,
    ) -> Self {
        let reversible = strategy.is_reversible();
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

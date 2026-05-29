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
use crate::policy::RuleRank;

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

/// A per-entity redaction record: what strategy was chosen, what
/// the original and replacement values are, and optional human
/// review.
///
/// Created by the policy evaluator via the builder, then enriched
/// by the applicator with the replacement value and `is_applied`
/// flag.
///
/// The entry lives next to its [`Entity<M>`] inside an
/// [`EntityRecord<M>`]; the entity's `id` and `location` are read
/// directly through the record rather than duplicated here.
///
/// [`Entity<M>`]: crate::entity::Entity
/// [`EntityRecord<M>`]: super::EntityRecord
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
    /// Identifier of the policy that triggered this redaction.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<Uuid>,
    /// Position of the producing rule in the per-run policy chain.
    /// Used by the codec at merge time to break ties when two
    /// overlapping redactions share the same [`LeakProfile`] and
    /// method — lower rank wins. `None` when the decision came from
    /// a source outside the policy evaluator (e.g. default
    /// threshold path).
    ///
    /// [`LeakProfile`]: crate::modality::LeakProfile
    #[builder(default, setter(into = false))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<RuleRank>,
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
    ///
    /// **Write-time snapshot** of `M::Strategy::is_reversible()` at
    /// the moment the audit entry was written, *not* a guarantee
    /// the original can still be recovered today. Strategies that
    /// depend on external state (vault tokens whose keys are
    /// revoked, encryption keys rotated out of the key provider,
    /// referenced cipher material that's been garbage-collected)
    /// can become un-reversible after the fact while the audit
    /// entry still reads `true`. Treat it as historical provenance
    /// — "this redaction was reversible at write time" — and
    /// resolve actual recoverability through the live key /
    /// vault state at read time.
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
    /// Set the strategy and original value in one call, deriving
    /// the `reversible` flag from the strategy.
    pub fn for_redaction(self, strategy: M::Strategy, original: impl Into<String>) -> Self {
        let reversible = strategy.is_reversible();
        self.with_redaction(RedactionSpec {
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

//! Audit entry: per-entity redaction record.
//!
//! [`AuditEntry<M>`] bundles three sub-records into one row of the
//! audit:
//!
//! - [`Decision<M>`] — what the policy evaluator chose (strategy,
//!   originating rule, the recogniser-extracted text). Immutable
//!   after evaluation.
//! - [`Execution<M>`] — what the codec applicator did (still
//!   pending, applied with an `M::Replacement`, failed with a
//!   reason, or explicitly suppressed). Mutated by the applicator;
//!   the variants form a single state machine that replaces the
//!   pre-reshape combination of `AuditEntryStatus` +
//!   `RedactionSpec.is_applied` + `RedactionValue.replacement`.
//! - [`EntryMetadata`] — when, correlation, optional review.

use derive_builder::Builder;
use jiff::Timestamp;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::modality::Modality;
use crate::policy::RuleRank;

/// A per-entity redaction record produced during a pipeline run.
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
        serialize = "M::Strategy: Serialize, M::Replacement: Serialize",
        deserialize = "M::Strategy: DeserializeOwned, M::Replacement: DeserializeOwned",
    )
)]
#[schemars(bound = "M::Strategy: JsonSchema, M::Replacement: JsonSchema")]
pub struct AuditEntry<M: Modality> {
    /// What the policy evaluator chose for this entity.
    pub decision: Decision<M>,
    /// What the codec applicator did (or didn't).
    #[builder(default = "Execution::Pending")]
    pub execution: Execution<M>,
    /// Timestamp, correlation, optional review.
    #[builder(default)]
    pub metadata: EntryMetadata,
}

impl<M: Modality> AuditEntry<M> {
    /// Start building a new audit entry.
    pub fn builder() -> AuditEntryBuilder<M> {
        AuditEntryBuilder::default()
    }
}

/// What the policy evaluator chose for an entity. Immutable after
/// evaluation; the applicator only writes to [`AuditEntry::execution`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(
    rename_all = "camelCase",
    bound(
        serialize = "M::Strategy: Serialize",
        deserialize = "M::Strategy: DeserializeOwned",
    )
)]
#[schemars(bound = "M::Strategy: JsonSchema")]
pub struct Decision<M: Modality> {
    /// Identifier of the policy that produced this decision. `None`
    /// when the decision came from a source outside the policy chain
    /// (e.g. the default-threshold fallback path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<Uuid>,
    /// Position of the producing rule in the per-run policy chain.
    /// Used by the codec at merge time to break ties when two
    /// overlapping redactions share the same `LeakProfile` and
    /// method. `None` for non-policy-driven decisions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<RuleRank>,
    /// Redaction strategy the evaluator picked.
    pub strategy: M::Strategy,
    /// Text the recogniser saw at the entity's location, captured at
    /// decision time. For text/tabular this is the source text; for
    /// image/audio it is the OCR/STT transcript at that location.
    /// May differ from the full entity value depending on the
    /// strategy's target.
    pub detected_text: String,
}

/// State machine for what the codec applicator did with a
/// [`Decision`]. The discriminant is the single source of truth for
/// "did the redaction run, and if so what happened" — there is no
/// parallel `is_applied` flag.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "status",
    rename_all = "snake_case",
    bound(
        serialize = "M::Replacement: Serialize",
        deserialize = "M::Replacement: DeserializeOwned",
    )
)]
#[schemars(bound = "M::Replacement: JsonSchema")]
pub enum Execution<M: Modality> {
    /// Decision recorded; applicator hasn't run yet.
    Pending,
    /// Applicator ran successfully. `replacement` records *what the
    /// codec wrote* at the entity's location, in the modality's
    /// per-`M::Replacement` shape.
    Applied { replacement: M::Replacement },
    /// Strategy conversion or codec apply errored. The decision
    /// stays on the entry; this variant records why no bytes were
    /// written.
    Failed { reason: String },
    /// A `Suppress` rule fired: the entity was deliberately not
    /// redacted. The decision's `strategy` is recorded for
    /// completeness but no codec work was scheduled.
    Suppressed,
}

impl<M: Modality> Execution<M> {
    /// `true` when the applicator finished and wrote a replacement.
    pub fn is_applied(&self) -> bool {
        matches!(self, Self::Applied { .. })
    }

    /// `true` when no apply work has been attempted yet.
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }
}

/// Per-entry timestamp + correlation. Separate from [`Decision`] /
/// [`Execution`] so consumers reading the "what happened" axis
/// don't pay for fields they don't need.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntryMetadata {
    /// When this entry was created.
    #[serde(default)]
    #[schemars(with = "Option<String>")]
    pub timestamp: Option<Timestamp>,
    /// Correlation identifier for tracing across services.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Uuid>,
}

impl EntryMetadata {
    /// Empty metadata stamped with the current wall-clock time.
    pub fn now() -> Self {
        Self {
            timestamp: Some(Timestamp::now()),
            correlation_id: None,
        }
    }
}

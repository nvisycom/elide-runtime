//! Audit entry: per-entity redaction record.
//!
//! [`AuditEntry<M>`] bundles three sub-records into one row of the
//! audit:
//!
//! - [`Decision<M>`] — what the policy evaluator chose (the rule
//!   that fired, where it ranks in the chain, and — for `Redact` —
//!   the operator spec the rule carried). Immutable after evaluation.
//! - [`Execution<M>`] — what the codec applicator did (still
//!   pending, applied with an `M::Replacement`, failed with a
//!   reason, or explicitly suppressed). Mutated by the applicator.
//! - [`EntryMetadata`] — when, correlation, optional review.

use derive_builder::Builder;
use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::override_decision::RedactionDecision;
use crate::modality::DocumentModality;
use crate::policy::{Action, RuleRank};

/// A per-entity redaction record produced during a pipeline run.
///
/// The entry lives next to its [`Entity<M>`] inside an
/// [`EntityRecord<M>`]; the entity's `id` and `location` are read
/// directly through the record rather than duplicated here.
///
/// [`Entity<M>`]: nvisy_core::entity::Entity
/// [`EntityRecord<M>`]: super::EntityRecord
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "AuditEntryBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry<M: DocumentModality> {
    /// What the policy evaluator chose for this entity.
    pub decision: Decision<M>,
    /// What the codec applicator did (or didn't).
    #[builder(default = "Execution::Pending")]
    pub execution: Execution<M>,
    /// Timestamp, correlation, optional review.
    #[builder(default)]
    pub metadata: EntryMetadata,
}

impl<M: DocumentModality> AuditEntry<M> {
    /// Start building a new audit entry.
    pub fn builder() -> AuditEntryBuilder<M> {
        AuditEntryBuilder::default()
    }
}

/// What the policy evaluator chose for an entity. Immutable after
/// evaluation; the applicator only writes to [`AuditEntry::execution`].
///
/// The full [`Action<M>`] from the winning rule is carried verbatim
/// so the audit record names the operator spec the apply phase ran
/// (or would have run). Callers reading audits can render the
/// operator without re-resolving the policy chain.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Decision<M: DocumentModality> {
    /// Identifier of the policy that produced this decision. `None`
    /// when the decision came from a source outside the policy chain
    /// (e.g. the default-threshold fallback path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<Uuid>,
    /// Position of the producing rule in the per-run policy chain.
    /// `None` for non-policy-driven decisions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<RuleRank>,
    /// The action the matching rule picked (with its operator spec
    /// for [`Action::Redact`]).
    pub action: Action<M>,
}

/// State machine for what the codec applicator did with a
/// [`Decision`]. The discriminant is the single source of truth for
/// "did the redaction run, and if so what happened" — there is no
/// parallel `is_applied` flag.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Execution<M: DocumentModality> {
    /// Decision recorded; applicator hasn't run yet.
    Pending,
    /// Applicator ran successfully. `replacement` records *what the
    /// operator wrote* at the entity's location, in the modality's
    /// per-`M::Replacement` shape.
    Applied { replacement: M::Replacement },
    /// Operator dispatch or codec apply errored. The decision stays
    /// on the entry; this variant records why no bytes were written.
    Failed { reason: String },
    /// A `Suppress` rule fired: the entity was deliberately not
    /// redacted. The decision is recorded for completeness but no
    /// codec work was scheduled.
    Suppressed,
}

impl<M: DocumentModality> Execution<M> {
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
    /// Provenance of the decision on this entry. `None` means
    /// the decision came from the policy chain with no override
    /// involvement (legacy unified runs). [`Engine::redact`]
    /// stamps every entry with an explicit tag — `PolicyChain`
    /// when no override touched the entity, or the matching
    /// `Override*` variant when one did.
    ///
    /// [`Engine::redact`]: crate::pipeline::Engine::redact
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_decision: Option<RedactionDecision>,
}

impl EntryMetadata {
    /// Empty metadata stamped with the current wall-clock time.
    pub fn now() -> Self {
        Self {
            timestamp: Some(Timestamp::now()),
            correlation_id: None,
            override_decision: None,
        }
    }

    /// Stamp the metadata with an override-provenance tag.
    /// Returns `self` for chained construction.
    #[must_use]
    pub fn with_override(mut self, decision: RedactionDecision) -> Self {
        self.override_decision = Some(decision);
        self
    }
}

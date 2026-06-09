//! Per-entity overrides applied during [`Engine::redact`].
//!
//! Four variants cover the human-in-the-loop review surface:
//!
//! - [`RedactionOverride::Accept`] — explicit approve. Implicit
//!   for any entity not mentioned in the overrides list, so
//!   `Accept` is rarely needed in practice but available for
//!   completeness.
//! - [`RedactionOverride::Reject`] — skip this entity entirely.
//!   The audit retains the entity but `Execution` becomes
//!   `Suppressed` instead of running an operator. The audit
//!   records the override provenance so a reviewer can see "a
//!   human chose to skip this entity."
//! - [`RedactionOverride::Replace`] — substitute a different
//!   operator for this entity, bypassing the policy chain's pick.
//!   The audit records both the original chain pick and the
//!   substituted operator.
//! - [`RedactionOverride::Add`] — inject an entity the
//!   recognisers missed. The redaction synthesises a typed
//!   [`Entity<M>`] (UUID minted fresh) and runs the policy chain
//!   for that entity exactly as if a recogniser had found it.
//!   When `operator` is `Some` the override pins that operator
//!   instead of trusting the chain.
//!
//! # Entity identity
//!
//! [`Accept`], [`Reject`], and [`Replace`] reference an entity by
//! its `Entity::id` (the per-mention UUID). Two coreferent mentions
//! of the same real-world entity each have distinct ids; an
//! override targets exactly one mention. To override every mention
//! of a coreferent entity, submit one override per mention.
//!
//! This is intentional: a reviewer can reject one mention while
//! accepting another (e.g. when the recogniser linked two
//! independent people under one coreference id).
//!
//! [`Accept`]: RedactionOverride::Accept
//! [`Reject`]: RedactionOverride::Reject
//! [`Replace`]: RedactionOverride::Replace
//! [`Engine::redact`]: super::super::Engine::redact
//! [`Entity<M>`]: nvisy_core::entity::Entity

use std::collections::HashSet;

use nvisy_codec::core::ModalityKind;
use nvisy_core::entity::EntityKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::modality::AnyLocation;
use crate::policy::AnyRedaction;

/// A single per-entity override carried by [`RedactionInput`].
///
/// [`RedactionInput`]: super::RedactionInput
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RedactionOverride {
    /// Approve the policy chain's decision for this entity.
    /// Equivalent to the default behaviour when an entity is not
    /// mentioned in the overrides list; present for completeness
    /// so callers can emit an explicit "I reviewed and approve"
    /// record.
    Accept {
        /// `Entity::id` from the detection's audit. Targets a
        /// single mention; coreferent mentions are independent.
        entity_id: Uuid,
    },
    /// Skip this entity. Execution becomes `Suppressed`; no
    /// operator runs.
    Reject {
        /// `Entity::id` from the detection's audit. Targets a
        /// single mention; coreferent mentions are independent.
        entity_id: Uuid,
    },
    /// Replace the operator the policy chain picked.
    Replace {
        /// `Entity::id` from the detection's audit. Targets a
        /// single mention; coreferent mentions are independent.
        entity_id: Uuid,
        /// Operator to apply instead of the policy chain's pick.
        /// Must match the entity's modality.
        operator: AnyRedaction,
    },
    /// Inject an entity the recognisers missed. The redaction
    /// engine synthesises a typed [`Entity<M>`] for it with a
    /// fresh UUID and runs the policy chain as if a recogniser
    /// had found it. The audit records this entity with
    /// `provenance: Override::Added`.
    ///
    /// [`Entity<M>`]: nvisy_core::entity::Entity
    Add(RedactionAddEntity),
}

impl RedactionOverride {
    /// The `Entity::id` this override targets. `None` for `Add`
    /// (the entity does not yet exist; the redaction mints a
    /// fresh UUID).
    #[must_use]
    pub fn target(&self) -> Option<Uuid> {
        match self {
            Self::Accept { entity_id }
            | Self::Reject { entity_id }
            | Self::Replace { entity_id, .. } => Some(*entity_id),
            Self::Add(_) => None,
        }
    }
}

/// Payload for [`RedactionOverride::Add`]. The location's modality
/// tag carries the entity's modality — there is no parallel
/// `modality` field, since two such fields could disagree.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionAddEntity {
    /// Location of the entity within the document. The variant
    /// tag (`text`/`tabular`/`image`/`audio`) determines the
    /// entity's modality.
    pub location: AnyLocation,
    /// Entity kind (drives policy evaluation and operator pick).
    pub entity_kind: EntityKind,
    /// When `Some`, pins the operator the redaction will apply,
    /// bypassing the policy chain for this entity. Must match
    /// `location`'s modality; mismatches are rejected at
    /// [`Engine::redact`] validation time.
    ///
    /// [`Engine::redact`]: super::super::Engine::redact
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator: Option<AnyRedaction>,
}

impl RedactionAddEntity {
    /// The modality the added entity belongs to. Derived from
    /// the location's variant tag.
    #[must_use]
    pub fn modality(&self) -> ModalityKind {
        self.location.kind()
    }
}

/// Validate a list of overrides for internal consistency.
///
/// Catches the bug classes a client could submit that the engine
/// would otherwise need to handle defensively:
///
/// - **Duplicate targets** — two overrides for the same
///   `entity_id`. Order would silently determine which wins; we
///   reject up front.
/// - **Modality mismatch** — a `Replace` whose `operator`'s
///   modality differs from the targeted entity (caught later by
///   the engine since the entity-id lookup is needed) and an
///   `Add` whose `operator`'s modality differs from the
///   `location`'s modality (catchable here).
///
/// Returns the first violation encountered; callers fix and
/// resubmit rather than iterating.
///
/// # Errors
///
/// [`ErrorKind::Validation`] with a message describing the
/// violation and the offending override index.
///
/// [`ErrorKind::Validation`]: nvisy_core::ErrorKind::Validation
pub fn validate_overrides(overrides: &[RedactionOverride]) -> Result<(), nvisy_core::Error> {
    const COMPONENT: &str = "nvisy_engine::pipeline::redaction::override";

    let mut seen: HashSet<Uuid> = HashSet::with_capacity(overrides.len());
    for (idx, ov) in overrides.iter().enumerate() {
        if let Some(target) = ov.target()
            && !seen.insert(target)
        {
            return Err(nvisy_core::Error::validation(
                format!("override #{idx} duplicates target entity {target}"),
                COMPONENT,
            ));
        }
        if let RedactionOverride::Add(add) = ov
            && let Some(op) = &add.operator
            && op.modality() != add.modality()
        {
            return Err(nvisy_core::Error::validation(
                format!(
                    "override #{idx} (add) operator modality {:?} does not match location modality {:?}",
                    op.modality(),
                    add.modality(),
                ),
                COMPONENT,
            ));
        }
    }
    Ok(())
}

// `RedactionDecision` lives in `provenance/override_decision.rs`
// because it's a property of the audit entry, not a property of
// the request shape. Re-exported from this module for ergonomic
// access:
pub use crate::provenance::RedactionDecision;

//! [`Landing`]: what an edit does to the report.
//!
//! Flattened away from the modality-generic [`Edit`] so it can
//! outlive the borrow that produced it — an `Edit<M>` cannot be
//! cloned out, because its derive would need `M: Clone` and a
//! modality marker is not.

use elide::Report;
use elide::entity::audit::{Attribution, AuditLog, ManualIntent};
use elide::entity::{Entity, LabelRef};
use elide::modality::Modality;
use elide::primitive::Confidence;
use elide_governance::modality::RedactableModality;
use uuid::Uuid;

use super::suppression::Suppression;
use crate::Edit;

/// What an edit does to the report, flattened away from the
/// modality-generic [`Edit`] so it can outlive the borrow that
/// produced it.
pub(super) enum Landing<M: Modality> {
    /// Append a detection recognition missed.
    Add {
        label: LabelRef,
        location: M::Location,
        reason: Option<String>,
        actor: Option<String>,
    },
    /// Rewrite what an existing detection is or covers, before the
    /// policy set sees it.
    Retag {
        id: Uuid,
        label: Option<LabelRef>,
        location: Option<M::Location>,
        reason: Option<String>,
        actor: Option<String>,
    },
    /// Stamp the suppression flag elide's redaction pass reads.
    Suppress {
        id: Uuid,
        reason: Option<String>,
        actor: Option<String>,
    },
    /// Lift an earlier *applied* suppression: a reviewer changing
    /// their mind across a round trip, with both halves kept on the
    /// trail.
    Unsuppress { id: Uuid },
}

impl<M: Modality> Landing<M> {
    /// Every edit maps to a landing: a redact's is `Unsuppress`,
    /// which lifts a suppression applied on an earlier pass — the
    /// operator itself reaches the anonymizer, not the report.
    pub(super) fn of<R: RedactableModality<Location = M::Location>>(edit: &Edit<R>) -> Self {
        match edit {
            Edit::Add(e) => Self::Add {
                label: e.label.clone(),
                location: e.location.clone(),
                reason: e.by.reason.clone(),
                actor: e.by.actor.clone(),
            },
            Edit::Retag(e) => Self::Retag {
                id: e.id,
                label: e.label.clone(),
                location: e.location.clone(),
                reason: e.by.reason.clone(),
                actor: e.by.actor.clone(),
            },
            Edit::Suppress(e) => Self::Suppress {
                id: e.id,
                reason: e.by.reason.clone(),
                actor: e.by.actor.clone(),
            },
            Edit::Redact(e) => Self::Unsuppress { id: e.id },
        }
    }

    /// Carry this edit out against `report`, reporting whether it
    /// found its target.
    ///
    /// `false` when the named entity is not in this report — it may
    /// belong to another modality, which each get their own pass, or
    /// the id may simply be stale. Not fatal here, but the caller
    /// must not treat an edit that changed nothing as applied.
    pub(super) fn land(self, report: &mut Report) -> bool {
        match self {
            Self::Add {
                label,
                location,
                reason,
                actor,
            } => {
                // `Entity::new` mints a v7 id, so a client can
                // neither forge one that shadows a real detection
                // nor pass its addition off as automatic: the `Flag`
                // is what marks the entity human-sourced.
                let mut entity = Entity::new(label, location, Confidence::MAX, AuditLog::default());
                entity.record_manual(
                    ManualIntent::Flag,
                    reason.map(Attribution::freeform).map(Into::into),
                    actor.as_deref(),
                );
                report.include::<M>(entity)
            }
            Self::Retag {
                id,
                label,
                location,
                reason,
                actor,
            } => {
                let Some(entity) = report.entity_anywhere_mut::<M>(id) else {
                    return false;
                };
                if let Some(label) = label {
                    entity.label = label;
                }
                if let Some(location) = location {
                    entity.location = location;
                }
                // Recorded against the *corrected* entity, which is
                // what the policy set matches on. `Amend` is
                // provenance-only: `is_suppressed` skips it, so a
                // correction after a suppression does not revive the
                // entity.
                entity.record_manual(
                    ManualIntent::Amend,
                    reason.map(Attribution::freeform).map(Into::into),
                    actor.as_deref(),
                );
                true
            }
            Self::Suppress { id, reason, actor } => {
                let Some(entity) = report.entity_anywhere_mut::<M>(id) else {
                    return false;
                };
                Suppression::On { reason, actor }.reconcile(entity);
                true
            }
            Self::Unsuppress { id } => {
                let Some(entity) = report.entity_anywhere_mut::<M>(id) else {
                    return false;
                };
                Suppression::Off.reconcile(entity);
                true
            }
        }
    }
}

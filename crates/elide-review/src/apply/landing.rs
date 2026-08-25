//! [`Landing`]: what an edit does to the report.
//!
//! Flattened away from the modality-generic [`Edit`] so it can
//! outlive the borrow that produced it — an `Edit<M>` cannot be
//! cloned out, because its derive would need `M: Clone` and a
//! modality marker is not.

use elide::Report;
use elide::entity::audit::{AuditEvent, AuditLog};
use elide::entity::{Entity, LabelRef};
use elide::modality::Modality;
use elide::primitive::Confidence;
use elide_governance::modality::RedactableModality;
use uuid::Uuid;

use super::entity::entity_mut;
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
    /// Every edit lands on the report except a redact, which only
    /// reaches the anonymizer — its `Unsuppress` arm exists to lift
    /// a suppression applied on an earlier pass.
    pub(super) fn of<R: RedactableModality<Location = M::Location>>(
        edit: &Edit<R>,
    ) -> Option<Self> {
        Some(match edit {
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
        })
    }

    /// Carry this edit out against `report`.
    ///
    /// An edit naming an entity the report does not hold is ignored
    /// rather than fatal: the entity may belong to another modality,
    /// or to a part this pass is not walking.
    pub(super) fn land(self, report: &mut Report) {
        match self {
            Self::Add {
                label,
                location,
                reason,
                actor,
            } => {
                // `Entity::new` mints a v7 id and this `Manual` event
                // is what marks the entity human-sourced, so a client
                // can neither forge an id that shadows a real
                // detection nor pass its addition off as automatic.
                let mut event = AuditEvent::manual_include(location.clone(), Confidence::MAX);
                if let Some(reason) = reason {
                    event = event.with_reason(reason);
                }
                if let Some(actor) = actor {
                    event = event.with_actor(actor);
                }
                let entity = Entity::new(label, location, Confidence::MAX, AuditLog::new(event));
                report.include::<M>(entity);
            }
            Self::Retag {
                id,
                label,
                location,
                reason,
                actor,
            } => {
                let Some(entity) = entity_mut::<M>(report, id) else {
                    return;
                };
                if let Some(label) = label {
                    entity.label = label;
                }
                if let Some(location) = location {
                    entity.location = location;
                }
                // `reason`/`actor` are deliberately not recorded
                // here. The only event kind that carries them is
                // `Manual`, whose `intent` is what `is_suppressed`
                // reads — stamping an `Include` for a correction
                // would silently un-suppress a suppressed entity.
                // A retag stays attributable through the edit
                // itself, which the caller holds until it applies.
                let _ = (reason, actor);
            }
            Self::Suppress { id, reason, actor } => {
                let Some(entity) = entity_mut::<M>(report, id) else {
                    return;
                };
                Suppression::On { reason, actor }.reconcile(entity);
            }
            Self::Unsuppress { id } => {
                let Some(entity) = entity_mut::<M>(report, id) else {
                    return;
                };
                Suppression::Off.reconcile(entity);
            }
        }
    }
}

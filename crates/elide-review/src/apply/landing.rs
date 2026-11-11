//! [`Landing`]: what an edit does to the report.
//!
//! Flattened away from the modality-generic [`Edit`] so it can
//! outlive the borrow that produced it — an `Edit<M>` cannot be
//! cloned out, because its derive would need `M: Clone` and a
//! modality marker is not.

use elide::entity::LabelRef;
use elide::entity::audit::{Attribution, Manual, ManualIntent};
use elide::modality::Modality;
use elide::{PartId, Report};
use elide_schema::modality::RedactableModality;
use uuid::Uuid;

use crate::Edit;

/// What an edit does to the report, flattened away from the
/// modality-generic [`Edit`] so it can outlive the borrow that
/// produced it.
pub(super) enum Landing<M: Modality> {
    /// Append a detection recognition missed.
    Add {
        label: LabelRef,
        location: M::Location,
        part: Option<Vec<String>>,
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
}

impl<M: Modality> Landing<M> {
    /// Every edit maps to a landing: all four reach the report.
    pub(super) fn of<R: RedactableModality<Location = M::Location>>(edit: &Edit<R>) -> Self {
        match edit {
            Edit::Add(e) => Self::Add {
                label: e.label.clone(),
                location: e.location.clone(),
                part: e.part.clone(),
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
        }
    }

    /// Carry this edit out against `report`.
    ///
    /// An edit naming an entity the report does not hold is a
    /// no-op: it may belong to another modality, which each get
    /// their own pass, or the id may simply be stale.
    pub(super) fn land(self, report: &mut Report) {
        match self {
            Self::Add {
                label,
                location,
                part,
                reason,
                actor,
            } => {
                // A named part is a path; `None` means the sole
                // document. Validation has already rejected a part
                // the report does not hold, or holds as another
                // modality, so neither lookup fails here.
                let view = match part {
                    Some(part) => report.part_mut::<M>(&PartId::from_segments(part)),
                    None => report.document_mut::<M>(),
                };
                let Ok(mut view) = view else {
                    return;
                };
                if let Some(actor) = actor {
                    view = view.by(actor);
                }
                if let Some(reason) = reason {
                    view = view.because(Attribution::freeform(reason));
                }
                // The id is minted, never client-supplied, so an add
                // can neither shadow a real detection nor pass itself
                // off as automatic: the `Flag` elide records is what
                // marks the entity human-sourced.
                view.include(label, location);
            }
            Self::Retag {
                id,
                label,
                location,
                reason,
                actor,
            } => {
                let Some(entity) = report.entity_anywhere_mut::<M>(id) else {
                    return;
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
                let amend = decision::<M>(ManualIntent::Amend, entity.location.clone(), reason);
                entity.record_manual(amend, actor.as_deref());
            }
            Self::Suppress { id, reason, actor } => {
                let Some(entity) = report.entity_anywhere_mut::<M>(id) else {
                    return;
                };
                // elide skips a `Suppress` the trail already
                // carries, so re-applying a set does not stack
                // duplicate events.
                let suppress =
                    decision::<M>(ManualIntent::Suppress, entity.location.clone(), reason);
                entity.record_manual(suppress, actor.as_deref());
            }
        }
    }
}

/// A reviewer's `Manual` decision of `intent` at `location`, carrying
/// the reviewer's `reason` when given.
fn decision<M: Modality>(
    intent: ManualIntent,
    location: M::Location,
    reason: Option<String>,
) -> Manual<M> {
    let manual = Manual::new(intent, location);
    match reason {
        Some(reason) => manual.with_attribution(Attribution::freeform(reason)),
        None => manual,
    }
}

//! [`Landing`]: what an edit does to the report.
//!
//! Flattened away from the modality-generic [`Edit`] so it can
//! outlive the borrow that produced it — an `Edit<M>` cannot be
//! cloned out, because its derive would need `M: Clone` and a
//! modality marker is not.

use elide::entity::audit::{Attribution, ManualIntent};
use elide::entity::{Entity, LabelRef};
use elide::modality::Modality;
use elide::{PartId, Report};
use elide_governance::modality::RedactableModality;
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

/// The single top-level document's [`PartId`], or `None` when the
/// report holds zero or several.
///
/// elide keeps its own `sole_document_id` private, so this rebuilds
/// it over the public [`Report::part_ids`]: a top-level document is
/// a depth-1 path, and "sole" means exactly one of them.
fn sole_document_id(report: &Report) -> Option<PartId> {
    let mut tops = report.part_ids().filter(|(id, _)| id.depth() == 1);
    let (first, _) = tops.next()?;
    let first = first.clone();
    tops.next().is_none().then_some(first)
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
                // The id is minted, never client-supplied, so an
                // add can neither shadow a real detection nor pass
                // itself off as automatic: the `Flag` the builder
                // stamps is what marks the entity human-sourced,
                // and it carries the reviewer's name and reason on
                // that one event.
                let mut custom = Entity::<M>::custom(label, location);
                if let Some(actor) = actor {
                    custom = custom.by(actor);
                }
                if let Some(reason) = reason {
                    custom = custom.because(Attribution::freeform(reason));
                }
                let entity = custom.build();

                // A named part is a path; `None` means the sole
                // document. A multi-document request has none, and
                // validation rejects that before landing.
                let Some(target) = part
                    .map(PartId::from_segments)
                    .or_else(|| sole_document_id(report))
                else {
                    return;
                };
                report.include_part::<M>(&target, entity);
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
                entity.record_manual(
                    ManualIntent::Amend,
                    reason.map(Attribution::freeform).map(Into::into),
                    actor.as_deref(),
                );
            }
            Self::Suppress { id, reason, actor } => {
                let Some(entity) = report.entity_anywhere_mut::<M>(id) else {
                    return;
                };
                // elide skips a `Suppress` the trail already
                // carries, so re-applying a set does not stack
                // duplicate events.
                entity.record_manual(
                    ManualIntent::Suppress,
                    reason.map(Attribution::freeform).map(Into::into),
                    actor.as_deref(),
                );
            }
        }
    }
}

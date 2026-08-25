//! Applying reviewer decisions to a report.
//!
//! An [`EditSet`] records what a reviewer changed; this is where
//! those decisions reach the document. They land in two different
//! ways, because elide models one of them and not the other:
//!
//! - **Suppression** is stamped onto the entity's own audit trail,
//!   because that trail is what elide's redaction pass reads to
//!   decide what to skip. The reversal of a suppression is recorded
//!   too, rather than erased, so the trail keeps both halves of a
//!   reviewer's change of mind.
//! - **An operator override** is layered onto the anonymizer ahead
//!   of the policy rules, straight from the edit list, because
//!   elide re-resolves operators from live policy at apply time and
//!   has no per-entity override of its own.
//!
//! A retag is neither: it rewrites what the entity *is* before the
//! policy set sees it, and is applied where the report is edited.
//!
//! [`EditSet`]: super::EditSet

use elide::Report;
use elide::codec::PartId;
use elide::entity::audit::{AuditEvent, AuditLog};
use elide::entity::{Entity, LabelRef};
use elide::modality::Modality;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide::primitive::Confidence;
use elide_governance::modality::RedactableModality;
use uuid::Uuid;

use crate::{Edit, EditBucket, EditSet};

impl EditSet {
    /// Land every pending edit on the report.
    ///
    /// Three of the four reach the document here: an add appends a
    /// new entity, a retag rewrites what an existing one is, and a
    /// suppress stamps the trail elide's redaction pass reads. The
    /// fourth — an operator override — is left pending, because it
    /// belongs to the anonymizer rather than the report: a consumer
    /// reads the surviving [`Edit::Redact`]s after this returns.
    ///
    /// Applied edits are dropped from the pending list, because the
    /// entity's own trail now records them. Leaving them would make
    /// a reviewer's *next* decision look like it contradicts one
    /// already carried out — reversing an applied suppression is a
    /// change of mind across two passes, not a self-contradicting
    /// payload.
    ///
    /// Idempotent: an entity elide already sees as suppressed is
    /// left alone, so re-applying an audit does not stack duplicate
    /// events.
    pub fn apply(&mut self, report: &mut Report) {
        apply_for::<Text>(self, report);
        apply_for::<Tabular>(self, report);
        apply_for::<Image>(self, report);
        apply_for::<Audio>(self, report);
    }
}

fn apply_for<M: EditBucket + 'static>(edits: &mut EditSet, report: &mut Report) {
    // Reduced to plain data first so the borrow on `edits` ends
    // before it is mutated below: an `Edit<M>` cannot be cloned out
    // (its derive would need `M: Clone`, which a modality marker is
    // not).
    let pending: Vec<Landing<M>> = M::bucket(edits).iter().filter_map(Landing::of).collect();

    for landing in pending {
        landing.land(report);
    }

    // Applied edits become history: the entity's trail now carries
    // them, so leaving one pending would make a reviewer's *next*
    // decision look like it contradicts one already carried out.
    // Redacts stay — nothing stamps them here, and `overrides()`
    // reads them after this runs.
    M::bucket_mut(edits).retain(|edit| matches!(edit, Edit::Redact { .. }));
}

/// What an edit does to the report, flattened away from the
/// modality-generic [`Edit`] so it can outlive the borrow that
/// produced it.
enum Landing<M: Modality> {
    /// Append a detection recognition missed.
    Add {
        label: LabelRef,
        location: M::Location,
    },
    /// Rewrite what an existing detection is or covers, before the
    /// policy set sees it.
    Retag {
        id: Uuid,
        label: Option<LabelRef>,
        location: Option<M::Location>,
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
    fn of<R: RedactableModality<Location = M::Location>>(edit: &Edit<R>) -> Option<Self> {
        Some(match edit {
            Edit::Add {
                label, location, ..
            } => Self::Add {
                label: label.clone(),
                location: location.clone(),
            },
            Edit::Retag {
                id,
                label,
                location,
                ..
            } => Self::Retag {
                id: *id,
                label: label.clone(),
                location: location.clone(),
            },
            Edit::Suppress {
                id, reason, actor, ..
            } => Self::Suppress {
                id: *id,
                reason: reason.clone(),
                actor: actor.clone(),
            },
            Edit::Redact { id, .. } => Self::Unsuppress { id: *id },
        })
    }

    /// Carry this edit out against `report`.
    ///
    /// An edit naming an entity the report does not hold is ignored
    /// rather than fatal: the entity may belong to another modality,
    /// or to a part this pass is not walking.
    fn land(self, report: &mut Report) {
        match self {
            Self::Add { label, location } => {
                // `Entity::new` mints a v7 id and elide's `include`
                // stamps the `Manual` event, so a client can neither
                // forge an id that shadows a real detection nor pass
                // its addition off as an automatic one.
                let event = AuditEvent::manual_include(location.clone(), Confidence::MAX);
                let entity = Entity::new(label, location, Confidence::MAX, AuditLog::new(event));
                report.include::<M>(entity);
            }
            Self::Retag {
                id,
                label,
                location,
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

/// What an edit implies for elide's suppression flag, flattened
/// away from the modality-generic [`Edit`] so it can outlive the
/// borrow that produced it.
enum Suppression {
    /// Leave the entity alone, recording why and by whom.
    On {
        reason: Option<String>,
        actor: Option<String>,
    },
    /// Redact it after all: an earlier suppression, if any, is
    /// lifted.
    Off,
}

impl Suppression {
    /// Bring `entity`'s trail in line with this decision.
    ///
    /// A no-op when the trail already says what the decision says,
    /// so re-applying an audit does not stack duplicate events.
    /// Reversal records a `Manual` include rather than rewriting
    /// history: `is_suppressed` reads the most recent `Manual`
    /// event, so the trail keeps both halves of a change of mind.
    fn reconcile<M: Modality>(&self, entity: &mut Entity<M>) {
        if matches!(self, Self::On { .. }) == entity.is_suppressed() {
            return;
        }
        let location = entity.location.clone();
        let confidence = entity.confidence;
        match self {
            Self::On { reason, actor } => {
                let mut event = AuditEvent::manual_suppress(location, confidence);
                if let Some(reason) = reason {
                    event = event.with_reason(reason.clone());
                }
                if let Some(actor) = actor {
                    event = event.with_actor(actor.clone());
                }
                entity.suppress(event);
            }
            Self::Off => {
                entity
                    .audit
                    .record(AuditEvent::manual_include(location, confidence));
            }
        }
    }
}

/// Find an entity by id anywhere in the report: the body first,
/// then every container part.
///
/// A report indexes entities per location, not globally, so a
/// decision keyed only by entity id has to be looked for.
fn entity_mut<M: Modality>(report: &mut Report, id: Uuid) -> Option<&mut Entity<M>> {
    if report.entity_mut::<M>(id).is_some() {
        return report.entity_mut::<M>(id);
    }
    let part_ids: Vec<PartId> = report.part_ids().map(|(id, _)| id.clone()).collect();
    part_ids
        .into_iter()
        .find(|part| report.part_entity_mut::<M>(part, id).is_some())
        .and_then(move |part| report.part_entity_mut::<M>(&part, id))
}

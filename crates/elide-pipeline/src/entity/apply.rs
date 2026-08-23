//! Applying reviewer decisions to a report.
//!
//! A [`ReviewSet`] records what a reviewer decided; this is where
//! those decisions reach the document. They land in two different
//! ways, because elide models one of them and not the other:
//!
//! - **Suppression** is stamped onto the entity's own audit trail,
//!   because that trail is what elide's redaction pass reads to
//!   decide what to skip. The reversal of a suppression is recorded
//!   too, rather than erased, so the trail keeps both halves of a
//!   reviewer's change of mind.
//! - **An operator override** is compiled into an [`OverrideSet`]
//!   and layered onto the anonymizer ahead of the policy rules,
//!   because elide re-resolves operators from live policy at apply
//!   time and has no per-entity override of its own.
//!
//! A retag is neither: it rewrites what the entity *is* before the
//! policy set sees it, and is applied where the report is edited.
//!
//! [`ReviewSet`]: super::ReviewSet

use std::collections::HashMap;

use elide::Report;
use elide::codec::PartId;
use elide::entity::Entity;
use elide::entity::audit::AuditEvent;
use elide::modality::Modality;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide_governance::modality::RedactableModality;
use uuid::Uuid;

use super::{OverrideEntry, OverrideSet, Review, ReviewBucket};
use crate::Audit;

impl Audit {
    /// Stamp every pending [`Review::Suppress`] onto its entity's
    /// audit trail, so elide's redaction pass skips it.
    ///
    /// Idempotent: an entity elide already sees as suppressed is
    /// left alone, so re-applying an audit does not stack duplicate
    /// events. A decision that is no longer a suppression records
    /// the reversal instead: `is_suppressed` reads the most recent
    /// `Manual` event, so an include lifts an earlier suppress and
    /// the trail keeps both halves of the reviewer's change of mind.
    pub(crate) fn apply_suppressions(&mut self) {
        apply_suppressions_for::<Text>(self);
        apply_suppressions_for::<Tabular>(self);
        apply_suppressions_for::<Image>(self);
        apply_suppressions_for::<Audio>(self);
    }

    /// Compile the reviewer's operator overrides for the anonymize
    /// path, bucketed by modality.
    ///
    /// Only [`Review::Redact`] yields one: a suppression names no
    /// operator and is applied by stamping the entity itself, and a
    /// retag hands the corrected entity back to the policy set.
    pub(crate) fn collect_overrides(&self) -> OverrideSet {
        OverrideSet {
            text: overrides_for(&self.reviews.text),
            tabular: overrides_for(&self.reviews.tabular),
            image: overrides_for(&self.reviews.image),
            audio: overrides_for(&self.reviews.audio),
        }
    }
}

fn apply_suppressions_for<M: ReviewBucket + 'static>(audit: &mut Audit) {
    // Which entities carry a decision, and whether each one wants
    // suppression. Reduced to plain data first so the borrow on
    // `audit.reviews` ends before `audit.report` is mutated: a
    // `Review<M>` cannot be cloned out (its derive would need
    // `M: Clone`, which a modality marker is not).
    let decisions: Vec<(Uuid, Suppression)> = M::bucket(&audit.reviews)
        .iter()
        .map(|(id, review)| (*id, Suppression::of(review)))
        .collect();

    for (id, decision) in decisions {
        let Some(entity) = entity_mut::<M>(&mut audit.report, id) else {
            continue;
        };
        decision.reconcile(entity);
    }
}

/// What a review implies for elide's suppression flag, flattened
/// away from the modality-generic [`Review`] so it can outlive the
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
    fn of<M: RedactableModality>(review: &Review<M>) -> Self {
        match review {
            Review::Suppress { reason, actor } => Self::On {
                reason: reason.clone(),
                actor: actor.clone(),
            },
            Review::Redact { .. } | Review::Retag { .. } => Self::Off,
        }
    }

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

fn overrides_for<M: RedactableModality>(
    reviews: &HashMap<Uuid, Review<M>>,
) -> Vec<OverrideEntry<M>> {
    reviews
        .iter()
        .filter_map(|(entity_id, review)| match review {
            Review::Redact { policy_id, action } => Some(OverrideEntry {
                entity_id: *entity_id,
                policy_id: *policy_id,
                action: action.clone(),
            }),
            Review::Suppress { .. } | Review::Retag { .. } => None,
        })
        .collect()
}

//! Applying reviewer decisions to a report.
//!
//! An [`EditSet`] records what a reviewer changed; this is where
//! those decisions reach the document. They become one elide
//! [`Review`] of the report, which elide checks whole and then
//! records on the entities each edit touches, as `Manual` audit
//! events:
//!
//! - **An add** includes a new entity, born of the reviewer's
//!   decision, so it is never mistaken for an automatic detection.
//! - **A retag** relabels or relocates an entity before the policy
//!   set sees it, so the corrected entity is redacted as if it had
//!   been detected that way.
//! - **A suppression** leaves an entity unredacted.
//! - **A hidden-content decision**: a remove (and each item a
//!   remove-all reaches) confirms the item, which the hidden-content
//!   plan reads as a removal; a keep suppresses everything within
//!   the item.
//!
//! Who decided is not part of an edit: the side applying the review
//! names the actor, from however it authenticated the reviewer, so
//! a review cannot claim to be someone else's.
//!
//! Which operator runs is not a reviewer's to choose either: elide
//! re-resolves operators from live policy at apply time.
//!
//! [`EditSet`]: super::EditSet
//! [`Review`]: elide::Review

mod hidden;

use elide::entity::audit::Attribution;
use elide::modality::Modality;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide::{Orchestrator, PartId, Report, Review, ReviewPart};
use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::index::HiddenItems;
use crate::{Edit, EditBucket, EditError, EditSet};

impl EditSet {
    /// Apply every edit to `report`, as the reviewer `by`.
    ///
    /// `by` names who made the decisions, as the caller
    /// authenticated them; elide refuses a blank one. Every edit is
    /// checked before any is recorded, so a set that cannot land in
    /// full lands not at all.
    ///
    /// Applies under no review policy: every edit a well-formed set
    /// makes is accepted. A deployment that restricts what a
    /// reviewer may do builds the [`review`](Self::review) itself
    /// and applies it with an orchestrator carrying its policy.
    ///
    /// Applies once: the review is bound to the report as it is
    /// now, and applying it changes that.
    ///
    /// # Errors
    ///
    /// Returns the first [`EditError`] [`review`](Self::review)
    /// finds, or [`Refused`](EditError::Refused) when elide declines
    /// the review, having applied nothing either way.
    pub fn apply(&self, report: &mut Report, by: &str) -> Result<(), EditError> {
        let review = self.review(report)?;
        if review.is_empty() {
            return Ok(());
        }
        Ok(Orchestrator::new().review(report, &review, by)?)
    }

    /// The elide [`Review`] these edits make of `report`.
    ///
    /// [`validate`](Self::validate) runs first, and its error is
    /// returned instead: elide would refuse what it rejects, but in
    /// terms of its own edits rather than the reviewer's.
    ///
    /// # Errors
    ///
    /// Returns the first [`EditError`] found.
    pub fn review(&self, report: &Report) -> Result<Review, EditError> {
        let items = HiddenItems::of(report);
        self.validate_with(report, &items)?;
        let mut review = Review::new(report);
        record_for::<Text>(self, report, &mut review)?;
        record_for::<Tabular>(self, report, &mut review)?;
        record_for::<Image>(self, report, &mut review)?;
        record_for::<Audio>(self, report, &mut review)?;
        hidden::record(&self.hidden, report, &items, &mut review)?;
        Ok(review)
    }
}

/// Add one modality's edits to `review`.
fn record_for<M: EditBucket>(
    edits: &EditSet,
    report: &Report,
    review: &mut Review,
) -> Result<(), EditError>
where
    M::Location: Serialize + DeserializeOwned,
{
    for edit in M::bucket(edits) {
        let part = match edit.target() {
            Some(id) => part_of::<M>(report, id),
            None => added_to(report, edit),
        };
        // Validation has already rejected an edit whose entity or
        // part the report does not hold.
        let Some(part) = part else {
            continue;
        };
        let mut view = open::<M>(review, &part, edit.reason())?;
        match edit {
            Edit::Add(add) => {
                view.include(add.label.clone(), add.location.clone());
            }
            Edit::Retag(retag) => {
                if let Some(label) = &retag.label {
                    view.relabel(retag.id, label.clone());
                }
                if let Some(location) = &retag.location {
                    view.relocate(retag.id, location.clone());
                }
            }
            Edit::Suppress(suppress) => {
                view.suppress(suppress.id);
            }
        }
    }
    Ok(())
}

/// The part `review` edits as `M`, attributed to `reason` when one
/// was given.
fn open<'r, M: Modality>(
    review: &'r mut Review,
    part: &PartId,
    reason: Option<&str>,
) -> Result<ReviewPart<'r, M>, EditError>
where
    M::Location: Serialize + DeserializeOwned,
{
    let view = review.part::<M>(part)?;
    Ok(match reason {
        Some(reason) => view.because(Attribution::freeform(reason.to_owned())),
        None => view,
    })
}

/// The part an add names: its path, or the sole document when it
/// names none, which validation has checked holds the add's
/// modality.
fn added_to<M: EditBucket>(report: &Report, edit: &Edit<M>) -> Option<PartId> {
    let Edit::Add(add) = edit else {
        return None;
    };
    match &add.part {
        Some(path) => Some(PartId::from_segments(path.clone())),
        None => report
            .part_ids()
            .map(|(id, _)| id)
            .find(|id| id.depth() == 1)
            .cloned(),
    }
}

/// The part of `report` holding the `M` entity `id`.
fn part_of<M: Modality>(report: &Report, id: Uuid) -> Option<PartId> {
    report
        .part_ids()
        .map(|(part, _)| part)
        .find(|part| {
            report
                .part::<M>(part)
                .is_ok_and(|entities| entities.entity(id).is_some())
        })
        .cloned()
}

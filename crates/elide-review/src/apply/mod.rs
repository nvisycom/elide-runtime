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

mod landing;

use elide::Report;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;

use self::landing::Landing;
use crate::{EditBucket, EditSet};

impl EditSet {
    /// Land every edit on the report.
    ///
    /// An add appends a new entity, a retag rewrites what an
    /// existing one is, and a suppress or restore stamps the trail
    /// elide's redaction pass reads to decide what to skip.
    ///
    /// Takes `&self`: an edit set is the caller's own input, not
    /// state this crate keeps, so nothing is consumed and the
    /// caller still holds every edit it submitted.
    ///
    /// An edit naming an entity the report does not hold is
    /// skipped rather than fatal — the id may be stale, or belong
    /// to a modality this report has no group for.
    ///
    /// Not idempotent across calls. A repeated suppress is a no-op
    /// (elide guards it), but a repeated add appends a second
    /// entity and a repeated retag double-records its amendment.
    /// Apply one set once.
    pub fn apply(&self, report: &mut Report) {
        apply_for::<Text>(self, report);
        apply_for::<Tabular>(self, report);
        apply_for::<Image>(self, report);
        apply_for::<Audio>(self, report);
    }
}

fn apply_for<M: EditBucket + 'static>(edits: &EditSet, report: &mut Report) {
    for edit in M::bucket(edits) {
        Landing::<M>::of(edit).land(report);
    }
}

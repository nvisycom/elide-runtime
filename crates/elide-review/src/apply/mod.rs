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

mod entity;
mod landing;
mod suppression;

use elide::Report;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;

use self::landing::Landing;
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
    M::bucket_mut(edits).retain(|edit| matches!(edit, Edit::Redact(_)));
}

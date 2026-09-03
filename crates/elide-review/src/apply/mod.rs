//! Applying reviewer decisions to a report.
//!
//! An [`EditSet`] records what a reviewer changed; this is where
//! those decisions reach the document. All three land on the
//! report, each in its own way:
//!
//! - **An add** appends a new entity, stamped human-sourced so it
//!   is never mistaken for an automatic detection.
//! - **A retag** rewrites what an entity *is* before the policy set
//!   sees it, so the corrected entity is redacted as if it had been
//!   detected that way.
//! - **A suppression** is stamped onto the entity's own audit
//!   trail, because that trail is what elide's redaction pass reads
//!   to decide what to skip.
//!
//! Which operator runs is not a reviewer's to choose: elide
//! re-resolves operators from live policy at apply time, and an
//! `OperatorId` carries a name and version but no configuration —
//! so there is nowhere on a report to record one.
//!
//! [`EditSet`]: super::EditSet

mod landing;

use elide::Report;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;

use self::landing::Landing;
use crate::{EditBucket, EditError, EditSet};

impl EditSet {
    /// Land every edit on the report.
    ///
    /// An add appends a new entity, a retag rewrites what an
    /// existing one is, and a suppress stamps the trail elide's
    /// redaction pass reads to decide what to skip.
    ///
    /// Takes `&self`: an edit set is the caller's own input, not
    /// state this crate keeps, so nothing is consumed and the
    /// caller still holds every edit it submitted.
    ///
    /// [`validate`](Self::validate) runs first and its error is
    /// returned unapplied, so a set that cannot land in full lands
    /// not at all. Landing is silent by nature — elide's
    /// `include_part` returns `false` for an unknown part and a
    /// missing entity is simply not found — so validating here is
    /// what keeps a reviewer from being told a decision took effect
    /// when the document says otherwise. It is not a second set of
    /// rules; it is the same one, made impossible to skip.
    ///
    /// Not idempotent across calls. A repeated suppress is a no-op
    /// (elide guards it), but a repeated add appends a second
    /// entity and a repeated retag double-records its amendment.
    /// Apply one set once.
    ///
    /// # Errors
    ///
    /// Returns the first [`EditError`](crate::EditError) found,
    /// having applied nothing.
    pub fn apply(&self, report: &mut Report) -> Result<(), EditError> {
        self.validate(report)?;
        apply_for::<Text>(self, report);
        apply_for::<Tabular>(self, report);
        apply_for::<Image>(self, report);
        apply_for::<Audio>(self, report);
        Ok(())
    }
}

fn apply_for<M: EditBucket + 'static>(edits: &EditSet, report: &mut Report) {
    for edit in M::bucket(edits) {
        Landing::<M>::of(edit).land(report);
    }
}

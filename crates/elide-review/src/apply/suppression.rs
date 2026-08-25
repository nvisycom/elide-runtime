//! [`Suppression`]: what an edit implies for elide's suppression
//! flag.
//!
//! elide reads the entity's own trail, not our edit list, to decide
//! what the redaction pass skips — so a suppression has to be
//! stamped there, and reversing one has to be stamped too rather
//! than erased.

use elide::entity::Entity;
use elide::entity::audit::{Attribution, ManualIntent};
use elide::modality::Modality;

/// What an edit implies for elide's suppression flag, flattened
/// away from the modality-generic [`Edit`] so it can outlive the
/// borrow that produced it.
pub(super) enum Suppression {
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
    /// Idempotent, and directionally so: [`record_manual`] skips a
    /// `Suppress` the trail already carries, while a `Flag` always
    /// records — which is what keeps both halves of a reviewer's
    /// change of mind on the trail rather than erasing the first.
    ///
    /// [`record_manual`]: elide::entity::Entity::record_manual
    pub(super) fn reconcile<M: Modality>(&self, entity: &mut Entity<M>) {
        match self {
            Self::On { reason, actor } => {
                entity.record_manual(
                    ManualIntent::Suppress,
                    reason.clone().map(Attribution::freeform).map(Into::into),
                    actor.as_deref(),
                );
            }
            // Reversal records a `Flag`: the redaction pass reads
            // the most recent decision, so this lifts the earlier
            // suppression without rewriting it.
            Self::Off if entity.is_suppressed() => {
                entity.record_manual(ManualIntent::Flag, None, None);
            }
            Self::Off => {}
        }
    }
}

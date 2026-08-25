//! [`Suppression`]: what an edit implies for elide's suppression
//! flag.
//!
//! elide reads the entity's own trail, not our edit list, to decide
//! what the redaction pass skips — so a suppression has to be
//! stamped there, and reversing one has to be stamped too rather
//! than erased.

use elide::entity::Entity;
use elide::entity::audit::AuditEvent;
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
    /// A no-op when the trail already says what the decision says,
    /// so re-applying an audit does not stack duplicate events.
    /// Reversal records a `Manual` include rather than rewriting
    /// history: `is_suppressed` reads the most recent `Manual`
    /// event, so the trail keeps both halves of a change of mind.
    pub(super) fn reconcile<M: Modality>(&self, entity: &mut Entity<M>) {
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

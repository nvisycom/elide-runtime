//! The [`ApplyPatch`] trait and blanket implementations.

use super::DocumentEnvelope;

/// A value that can be applied to a [`DocumentEnvelope`], merging
/// operation results into the shared pipeline state.
///
/// Each operation returns a concrete patch type; the orchestrator
/// calls [`apply`](ApplyPatch::apply) to fold it into the envelope
/// without needing to know the operation's internals.
pub trait ApplyPatch {
    /// Merge this patch into the envelope.
    fn apply(self, envelope: &mut DocumentEnvelope);
}

/// A no-op patch for operations that don't modify the envelope.
impl ApplyPatch for () {
    fn apply(self, _envelope: &mut DocumentEnvelope) {}
}

/// Apply multiple patches of the same type in sequence.
impl<P: ApplyPatch> ApplyPatch for Vec<P> {
    fn apply(self, envelope: &mut DocumentEnvelope) {
        for patch in self {
            patch.apply(envelope);
        }
    }
}

impl<A: ApplyPatch, B: ApplyPatch> ApplyPatch for (A, B) {
    fn apply(self, envelope: &mut DocumentEnvelope) {
        self.0.apply(envelope);
        self.1.apply(envelope);
    }
}

impl<A: ApplyPatch, B: ApplyPatch, C: ApplyPatch> ApplyPatch for (A, B, C) {
    fn apply(self, envelope: &mut DocumentEnvelope) {
        self.0.apply(envelope);
        self.1.apply(envelope);
        self.2.apply(envelope);
    }
}

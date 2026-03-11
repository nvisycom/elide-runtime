//! Policy evaluation patches.

use super::DocumentEnvelope;
use super::apply::ApplyPatch;
use crate::provenance::{RedactionDecision, RedactionRecord};

/// Redaction decisions and audit records produced by policy evaluation.
pub struct PolicyOutcome {
    /// How each entity should be redacted.
    pub decisions: Vec<RedactionDecision>,
    /// Audit-facing records of what was decided.
    pub records: Vec<RedactionRecord>,
}

impl ApplyPatch for PolicyOutcome {
    fn apply(self, envelope: &mut DocumentEnvelope) {
        envelope.audit.decisions.extend(self.decisions);
        envelope.audit.records.extend(self.records);
    }
}

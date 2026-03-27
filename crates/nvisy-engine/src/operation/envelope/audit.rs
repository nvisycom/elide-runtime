//! Audit entry patches.

use super::DocumentEnvelope;
use super::apply::ApplyPatch;
use nvisy_ontology::provenance::AuditEntry;

/// A single audit log entry recording what an operation did.
pub struct OperationEntry(pub AuditEntry);

impl ApplyPatch for OperationEntry {
    fn apply(self, envelope: &mut DocumentEnvelope) {
        envelope.audit.push_entry(self.0);
    }
}

impl ApplyPatch for AuditEntry {
    fn apply(self, envelope: &mut DocumentEnvelope) {
        envelope.audit.push_entry(self);
    }
}

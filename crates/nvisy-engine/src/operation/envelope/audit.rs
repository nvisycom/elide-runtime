//! Audit entry patches.

use nvisy_ontology::provenance::AuditEntry;

use super::DocumentEnvelope;
use super::apply::ApplyPatch;

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

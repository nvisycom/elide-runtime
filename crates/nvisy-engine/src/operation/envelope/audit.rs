//! Audit entry patches.

use crate::provenance::AuditEntry;

use super::apply::ApplyPatch;
use super::DocumentEnvelope;

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

//! Per-file processing audit log.
//!
//! A [`FileAudit`] records every operation performed on a specific file:
//! models used, tokens consumed, data extracted, transformations applied.
//! This is distinct from the governance-level [`Audit`](nvisy_identify::Audit)
//! which tracks policy and redaction events.

mod entry;
mod inference;
mod kind;
mod lifecycle;
mod processing;

pub use entry::{AuditEntryStatus, FileAuditEntry, FileAuditEntryBuilder, FileAuditEntryBuilderError};
pub use inference::{InferenceAction, InferenceActionBuilder};
pub use kind::{FileAuditEntryKind, InferenceKind, LifecycleKind, ProcessingKind};
pub use lifecycle::{LifecycleAction, LifecycleActionBuilder};
pub use processing::{ProcessingAction, ProcessingActionBuilder};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use nvisy_core::path::ContentSource;

/// An append-only processing log for a single file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileAudit {
    /// Content source this audit belongs to.
    #[serde(flatten)]
    pub source: ContentSource,
    /// Ordered list of processing entries.
    pub entries: Vec<FileAuditEntry>,
}

impl FileAudit {
    /// Create a new empty audit log for the given source.
    pub fn new(source: ContentSource) -> Self {
        Self {
            source,
            entries: Vec::new(),
        }
    }

    /// Append an entry to the audit log.
    pub fn push(&mut self, entry: FileAuditEntry) {
        self.entries.push(entry);
    }

    /// Number of entries recorded.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no entries have been recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

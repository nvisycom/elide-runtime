//! Provenance: audit trails and per-file processing logs.
//!
//! [`FileAudit`] is an append-only per-file processing log recording every
//! operation performed on a specific file (models used, tokens consumed, etc.).

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

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nvisy_core::path::ContentSource;

/// An append-only processing log for a single file.
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "FileAuditBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
)]
#[serde(rename_all = "camelCase")]
pub struct FileAudit {
    /// Content source this audit belongs to.
    #[serde(flatten)]
    pub source: ContentSource,
    /// Identifier of the pipeline run that produced this audit.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<Uuid>,
    /// Identifier of the human or service account that triggered the run.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<Uuid>,
    /// Ordered list of processing entries.
    #[builder(default)]
    pub entries: Vec<FileAuditEntry>,
}

impl FileAudit {
    /// Create a new empty audit log for the given source.
    pub fn new(source: ContentSource) -> Self {
        Self {
            source,
            run_id: None,
            actor_id: None,
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

//! Provenance: audit trails, redaction records, and per-file processing logs.
//!
//! This module is the single source of truth for all pipeline audit and
//! decision types. It combines two concerns:
//!
//! - **Execution logs** ([`Audit`], [`AuditEntry`]): when operations
//!   ran, how long they took, what models were used, token counts, etc.
//!
//! - **Redaction records** ([`RedactionRecord`], [`PolicyEvaluation`]):
//!   what was redacted, why, and human-review status.
//!
//! Together these form a complete audit trail for compliance and review.

mod entry;
mod kind;

mod action;
mod record;

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::action::{
    InferenceAction, InferenceActionBuilder, LifecycleAction, LifecycleActionBuilder,
    ProcessingAction, ProcessingActionBuilder,
};
pub use self::entry::{AuditEntry, AuditEntryBuilder, AuditEntryBuilderError, AuditEntryStatus};
pub use self::kind::{AuditEntryKind, InferenceKind, LifecycleKind, ProcessingKind};
pub use self::record::{
    PolicyEvaluation, RedactionLifecycle, RedactionRecord, RedactionRecordBuilder, RedactionSpec,
    RedactionValue, ReviewDecision, ReviewStatus,
};
use crate::entity::{ContentSource, Entities};

fn entities_empty(entities: &Entities) -> bool {
    entities.is_empty()
}

/// A per-document audit trail combining execution logs with redaction records.
///
/// `Audit` is the single compliance artifact for a document. It tracks:
/// - **What was found**: via [`entities`](Self::entities)
/// - **What operations ran**: via [`entries`](Self::entries)
/// - **What was redacted and how**: via [`records`](Self::records)
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "AuditBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    /// Content source this audit belongs to.
    pub source: ContentSource,
    /// Identifier of the pipeline run that produced this audit.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<Uuid>,
    /// Identifier of the human or service account that triggered the run.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<Uuid>,
    /// Entities detected during the pipeline run.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "entities_empty")]
    pub entities: Entities,
    /// Ordered list of processing entries.
    #[builder(default)]
    pub entries: Vec<AuditEntry>,
    /// Redaction records: strategy, original value, replacement, review status.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub records: Vec<RedactionRecord>,
}

impl Audit {
    /// Create a new empty audit for the given source.
    pub fn new(source: ContentSource) -> Self {
        Self {
            source,
            run_id: None,
            actor_id: None,
            entities: Entities::new(),
            entries: Vec::new(),
            records: Vec::new(),
        }
    }

    /// Append a processing entry.
    pub fn push_entry(&mut self, entry: AuditEntry) {
        self.entries.push(entry);
    }

    /// Append a redaction record.
    pub fn push_record(&mut self, record: RedactionRecord) {
        self.records.push(record);
    }

    /// Number of processing entries recorded.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no processing entries have been recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

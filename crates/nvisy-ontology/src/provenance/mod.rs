//! Provenance: audit trails and per-file processing logs.
//!
//! This module is the single source of truth for all pipeline audit
//! types:
//!
//! - [`AuditEntry`]: per-entity redaction record with strategy,
//!   original/replacement values, and optional review.
//! - [`PolicyEvaluation`]: aggregate outcome of evaluating a policy.
//! - [`Audit`]: per-document container for entities and audit entries.

mod entry;
mod evaluation;
mod review;

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::entry::{
    AuditEntry, AuditEntryBuilder, AuditEntryStatus, RedactionSpec, RedactionValue,
};
pub use self::evaluation::PolicyEvaluation;
pub use self::review::{ReviewDecision, ReviewStatus};
use crate::entity::{ContentSource, Entities};

fn entities_empty(entities: &Entities) -> bool {
    entities.is_empty()
}

/// A per-document audit trail: detected entities and redaction entries.
///
/// `Audit` is the single compliance artifact for a document. It tracks:
/// - **What was found**: via [`entities`](Self::entities)
/// - **What was redacted and how**: via [`entries`](Self::entries)
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
    /// Per-entity redaction audit entries.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<AuditEntry>,
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
        }
    }

    /// Append an audit entry.
    pub fn push_entry(&mut self, entry: AuditEntry) {
        self.entries.push(entry);
    }

    /// Number of audit entries recorded.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no audit entries have been recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

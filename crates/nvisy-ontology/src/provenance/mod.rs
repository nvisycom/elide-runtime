//! Provenance: audit trails and per-file processing logs.
//!
//! This module is the single source of truth for all pipeline audit
//! types:
//!
//! - [`AuditEntry<M>`]: per-entity redaction record with strategy,
//!   original/replacement values, and optional review.
//! - [`Audit<M>`]: per-document container for entities and audit
//!   entries.
//!
//! All provenance types are typed per modality. Cross-modality
//! aggregation (rich documents that process as multiple typed
//! envelopes) is the engine's responsibility — provenance stays
//! per-envelope.

mod entry;
mod redaction_map;
mod review;

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::entry::{
    AuditEntry, AuditEntryBuilder, AuditEntryStatus, RedactionSpec, RedactionValue,
};
pub use self::redaction_map::{RedactionMap, RedactionMapping};
pub use self::review::{ReviewDecision, ReviewStatus};
use crate::entity::{ContentSource, Entity};
use crate::modality::Modality;

/// A per-document audit trail: detected entities and redaction
/// entries.
///
/// `Audit<M>` is the compliance artifact for one typed document. It
/// tracks:
/// - **What was found**: via [`entities`]
/// - **What was redacted and how**: via [`entries`]
///
/// [`entities`]: Self::entities
/// [`entries`]: Self::entries
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "AuditBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(
    rename_all = "camelCase",
    bound(
        serialize = "M: Serialize, M::Strategy: Serialize",
        deserialize = "M: DeserializeOwned, M::Strategy: DeserializeOwned",
    )
)]
#[schemars(bound = "M: JsonSchema, M::Strategy: JsonSchema")]
pub struct Audit<M: Modality> {
    /// Content source this audit belongs to.
    pub source: ContentSource,
    /// Identifier of the pipeline run that produced this audit.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<Uuid>,
    /// Identifier of the human or service account that triggered the
    /// run.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<Uuid>,
    /// Entities detected during the pipeline run.
    #[builder(default = "Vec::new()")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<Entity<M>>,
    /// Per-entity redaction audit entries.
    #[builder(default = "Vec::new()")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<AuditEntry<M>>,
}

impl<M: Modality> Audit<M> {
    /// Create a new empty audit for the given source.
    pub fn new(source: ContentSource) -> Self {
        Self {
            source,
            run_id: None,
            actor_id: None,
            entities: Vec::new(),
            entries: Vec::new(),
        }
    }

    /// Append an audit entry.
    pub fn push_entry(&mut self, entry: AuditEntry<M>) {
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

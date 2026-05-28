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
use derive_more::{From, IsVariant};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::entry::{
    AuditEntry, AuditEntryBuilder, AuditEntryStatus, RedactionSpec, RedactionValue,
};
pub use self::redaction_map::{RedactionMap, RedactionMapping};
pub use self::review::{ReviewDecision, ReviewStatus};
use crate::entity::ContentSource;
use crate::entity::Entity;
use crate::modality::{Audio, Image, Modality, Tabular, Text};

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

/// A modality-erased [`Audit`].
///
/// The persistence layer ([`RegistryStore::store_audits`]) and the
/// engine's public output ([`EngineOutput::audits`]) work with
/// `Vec<AnyAudit>` so that a single multi-modal pipeline run can
/// surface audits for every modality it produced (e.g. a PDF that
/// fanned out into a `Text` envelope and an `Image` envelope returns
/// two audits — one of each modality).
///
/// Wire format: tagged by `modality`, flattening the audit's own
/// fields into the same JSON object:
///
/// ```json
/// { "modality": "text", "source": {...}, "entities": [...], "entries": [...] }
/// ```
///
/// [`RegistryStore::store_audits`]: https://docs.rs/nvisy-engine/latest/nvisy_engine/ingestion/struct.RegistryStore.html#method.store_audits
/// [`EngineOutput::audits`]: https://docs.rs/nvisy-engine/latest/nvisy_engine/pipeline/struct.EngineOutput.html#structfield.audits
#[derive(Debug, Clone, From, IsVariant, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "modality", rename_all = "snake_case")]
pub enum AnyAudit {
    Text(Audit<Text>),
    Tabular(Audit<Tabular>),
    Image(Audit<Image>),
    Audio(Audit<Audio>),
}

impl AnyAudit {
    /// Human-readable name of the contained modality. Useful for
    /// telemetry and error messages.
    pub fn modality_name(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Tabular(_) => "tabular",
            Self::Image(_) => "image",
            Self::Audio(_) => "audio",
        }
    }

    /// Content source the audit belongs to. Available without
    /// matching on the variant.
    pub fn source(&self) -> ContentSource {
        match self {
            Self::Text(a) => a.source,
            Self::Tabular(a) => a.source,
            Self::Image(a) => a.source,
            Self::Audio(a) => a.source,
        }
    }

    /// Pipeline-run identifier this audit belongs to, if any.
    pub fn run_id(&self) -> Option<Uuid> {
        match self {
            Self::Text(a) => a.run_id,
            Self::Tabular(a) => a.run_id,
            Self::Image(a) => a.run_id,
            Self::Audio(a) => a.run_id,
        }
    }

    /// Number of detected entities, summed across all variants.
    pub fn entities_count(&self) -> usize {
        match self {
            Self::Text(a) => a.entities.len(),
            Self::Tabular(a) => a.entities.len(),
            Self::Image(a) => a.entities.len(),
            Self::Audio(a) => a.entities.len(),
        }
    }

    /// Number of redaction audit entries.
    pub fn entries_count(&self) -> usize {
        match self {
            Self::Text(a) => a.entries.len(),
            Self::Tabular(a) => a.entries.len(),
            Self::Image(a) => a.entries.len(),
            Self::Audio(a) => a.entries.len(),
        }
    }

    /// Number of redaction audit entries whose redaction was actually
    /// applied (as opposed to suppressed or skipped). Used by the
    /// engine's run-summary counters.
    pub fn applied_redactions_count(&self) -> usize {
        fn count<M: Modality>(audit: &Audit<M>) -> usize {
            audit
                .entries
                .iter()
                .filter(|e| e.redaction.is_applied)
                .count()
        }
        match self {
            Self::Text(a) => count(a),
            Self::Tabular(a) => count(a),
            Self::Image(a) => count(a),
            Self::Audio(a) => count(a),
        }
    }
}


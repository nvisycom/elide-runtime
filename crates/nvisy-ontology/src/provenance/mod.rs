//! Provenance: audit trails and per-file processing logs.
//!
//! This module is the single source of truth for all pipeline audit
//! types:
//!
//! - [`AuditEntry<M>`]: per-entity redaction record with strategy,
//!   original/replacement values, and optional review.
//! - [`EntityRecord<M>`]: an [`Entity<M>`] bundled with its
//!   optional [`AuditEntry<M>`].
//! - [`Audit<M>`]: per-document container holding the records
//!   produced during a pipeline run.
//!
//! All provenance types are typed per modality. Cross-modality
//! aggregation (rich documents that process as multiple typed
//! envelopes) is the engine's responsibility — provenance stays
//! per-envelope.
//!
//! [`Entity<M>`]: crate::entity::Entity

mod entry;
mod record;
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
pub use self::record::EntityRecord;
pub use self::review::{ReviewDecision, ReviewStatus};
use crate::entity::ContentSource;
use crate::entity::Entity;
use crate::modality::{Audio, Image, Modality, Tabular, Text};

/// A per-document audit trail: per-entity records bundling the
/// detected entity with the audit entry (if any) produced for it
/// during the pipeline run.
///
/// `Audit<M>` is the compliance artifact for one typed document.
/// Each [`EntityRecord<M>`] in [`records`] represents one
/// detection; the record's `audit` field is `Some` when a
/// redaction or suppression rule matched.
///
/// [`records`]: Self::records
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
    /// Per-entity records produced during the pipeline run.
    #[builder(default = "Vec::new()")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub records: Vec<EntityRecord<M>>,
}

impl<M: Modality> Audit<M> {
    /// Create a new empty audit for the given source.
    pub fn new(source: ContentSource) -> Self {
        Self {
            source,
            run_id: None,
            actor_id: None,
            records: Vec::new(),
        }
    }

    /// Append a record for `entity` with no audit decision yet.
    pub fn push_entity(&mut self, entity: Entity<M>) {
        self.records.push(EntityRecord::new(entity));
    }

    /// Iterate over the detected entities (read-only).
    pub fn entities(&self) -> impl Iterator<Item = &Entity<M>> {
        self.records.iter().map(|r| &r.entity)
    }

    /// Iterate over the audit entries that have been recorded.
    pub fn entries(&self) -> impl Iterator<Item = &AuditEntry<M>> {
        self.records.iter().filter_map(|r| r.audit.as_ref())
    }

    /// Iterate over `(entity, audit_entry)` pairs for records that
    /// have a redaction decision.
    pub fn decided(&self) -> impl Iterator<Item = (&Entity<M>, &AuditEntry<M>)> {
        self.records
            .iter()
            .filter_map(|r| r.audit.as_ref().map(|a| (&r.entity, a)))
    }

    /// Number of detected entities.
    pub fn entities_count(&self) -> usize {
        self.records.len()
    }

    /// Number of audit entries recorded.
    pub fn entries_count(&self) -> usize {
        self.records.iter().filter(|r| r.audit.is_some()).count()
    }

    /// Number of redaction audit entries whose redaction was
    /// actually applied (as opposed to suppressed or skipped).
    pub fn applied_redactions_count(&self) -> usize {
        self.records
            .iter()
            .filter(|r| r.audit.as_ref().is_some_and(|a| a.redaction.is_applied))
            .count()
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
/// { "modality": "text", "source": {...}, "records": [...] }
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
            Self::Text(a) => a.entities_count(),
            Self::Tabular(a) => a.entities_count(),
            Self::Image(a) => a.entities_count(),
            Self::Audio(a) => a.entities_count(),
        }
    }

    /// Number of redaction audit entries.
    pub fn entries_count(&self) -> usize {
        match self {
            Self::Text(a) => a.entries_count(),
            Self::Tabular(a) => a.entries_count(),
            Self::Image(a) => a.entries_count(),
            Self::Audio(a) => a.entries_count(),
        }
    }

    /// Number of redaction audit entries whose redaction was actually
    /// applied (as opposed to suppressed or skipped). Used by the
    /// engine's run-summary counters.
    pub fn applied_redactions_count(&self) -> usize {
        match self {
            Self::Text(a) => a.applied_redactions_count(),
            Self::Tabular(a) => a.applied_redactions_count(),
            Self::Image(a) => a.applied_redactions_count(),
            Self::Audio(a) => a.applied_redactions_count(),
        }
    }
}

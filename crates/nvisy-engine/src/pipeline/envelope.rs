//! [`DocumentEnvelope<M>`] + [`AnyEnvelope`]: the orchestrator's
//! per-document carrier.
//!
//! An envelope is created at import and travels through every phase
//! of one document's pipeline. Rich sources (PDFs with both text and
//! image layers) produce one root `DocumentEnvelope<Text>` whose
//! document hosts nested image/tabular documents via
//! [`TextBlock::Embed`]; the shared codec [`SharedHandle`] on the
//! root envelope services reads and mutations for every nested doc.
//!
//! Phases never see the envelope directly — they operate on the
//! narrower [`PhaseTarget`] view the orchestrator builds from it per
//! phase iteration. The envelope is the orchestrator's bookkeeping
//! shape: it owns the handle + per-modality document + run-shared
//! state and is the unit of work spawned onto the per-document task
//! pool.
//!
//! [`TextBlock::Embed`]: nvisy_ontology::modality::TextBlock::Embed
//! [`SharedHandle`]: crate::core::SharedHandle
//! [`PhaseTarget`]: crate::core::PhaseTarget

use std::fmt;
use std::sync::Arc;

use derive_more::{From, IsVariant};
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentMetadata, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::document::Document;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::{Audio, Image, Modality, Tabular, Text};
use nvisy_ontology::provenance::AnyAudit;
use uuid::Uuid;

use crate::core::{SharedData, SharedHandle};

/// Per-document state for one modality that flows through the
/// pipeline.
pub struct DocumentEnvelope<M: Modality> {
    /// The codec handle for the document's bytes. Shared via
    /// `Arc<Mutex>` across modality-typed envelopes spawned from the
    /// same source so they can serialize reads and mutations to the
    /// underlying document.
    pub handle: SharedHandle,

    /// Content metadata (MIME type, filename, etc.) from the
    /// original upload.
    pub metadata: ContentMetadata,

    /// Per-modality document representation (text/image/audio/
    /// tabular). Populated empty at import; filled in by extraction
    /// (OCR/STT for image/audio, codec walk for text/tabular).
    /// Carries both structural content *and* the run-scoped audit
    /// on [`Document::audit`] so the recursive document tree
    /// (text doc with embedded image/tabular docs) keeps every
    /// nested doc's audit attached to the doc it describes.
    pub document: Document<M>,

    /// IDs of reference-data [`Context`]s loaded by [`LoadContext`]
    /// nodes. Each UUID resolves through the engine's context cache.
    ///
    /// [`Context`]: nvisy_ontology::context::Context
    /// [`LoadContext`]: crate::ingestion::LoadContext
    pub contexts: Vec<Uuid>,

    /// Run-wide shared state (policies, registry, key provider).
    /// Cheaply cloneable (`Arc`): all envelopes in a run share the
    /// same underlying data.
    pub shared: Arc<SharedData>,
}

impl<M: Modality> DocumentEnvelope<M> {
    /// Create a new envelope from a shared codec handle, metadata,
    /// and the per-modality document metadata. The document is
    /// initialised empty (no blocks, no annotations) and a fresh
    /// audit is opened against the handle's source.
    ///
    /// The caller (importer fan-out) supplies `document_meta` because
    /// the extraction tag and other per-modality metadata are
    /// importer-time knowledge — every envelope is born with an
    /// extraction path the importer just decided.
    pub async fn new(
        handle: SharedHandle,
        metadata: ContentMetadata,
        document_meta: M::Metadata,
        shared: Arc<SharedData>,
    ) -> Self {
        let source = handle.lock().await.source();
        let document = Document::new(document_meta, source);
        Self {
            handle,
            metadata,
            document,
            contexts: Vec::new(),
            shared,
        }
    }

    /// The document type of the underlying content.
    pub async fn document_type(&self) -> DocumentType {
        self.handle.lock().await.document_type()
    }

    /// Content source identity and lineage.
    pub async fn source(&self) -> ContentSource {
        self.handle.lock().await.source()
    }

    /// Encode the codec handle back to raw bytes.
    pub async fn encode(&self) -> Result<ContentData, Error> {
        self.handle.lock().await.encode()
    }

    /// Number of detected entities.
    pub fn entity_count(&self) -> usize {
        self.document.audit.records.len()
    }

    /// Add detected entities (wrapped into fresh [`EntityRecord`]s).
    ///
    /// Convenience forwarder to [`Document::add_entities`] for the
    /// common single-doc detection path; nested-doc detection calls
    /// the document method directly.
    ///
    /// [`EntityRecord`]: nvisy_ontology::provenance::EntityRecord
    /// [`Document::add_entities`]: nvisy_ontology::document::Document::add_entities
    pub fn add_entities(&mut self, entities: impl IntoIterator<Item = Entity<M>>) {
        self.document.add_entities(entities);
    }
}

impl<M: Modality> fmt::Debug for DocumentEnvelope<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocumentEnvelope")
            .field("records", &self.document.audit.records.len())
            .field("contexts", &self.contexts.len())
            .finish_non_exhaustive()
    }
}

/// A modality-erased envelope returned by the importer.
///
/// The importer produces one `AnyEnvelope` per uploaded file.
/// Single-modality content (txt, csv, png, wav, …) yields the
/// matching variant. Rich documents (PDF, DOCX) yield the `Text`
/// variant; their image and tabular content lives inside the root
/// [`Document<Text>`] as nested documents under [`TextBlock::Embed`]
/// children, populated by the image/tabular extraction steps from
/// the same shared codec handle.
///
/// [`Document<Text>`]: nvisy_ontology::document::Document
/// [`TextBlock::Embed`]: nvisy_ontology::modality::TextBlock::Embed
#[derive(Debug, From, IsVariant)]
pub enum AnyEnvelope {
    Text(DocumentEnvelope<Text>),
    Tabular(DocumentEnvelope<Tabular>),
    Image(DocumentEnvelope<Image>),
    Audio(DocumentEnvelope<Audio>),
}

impl AnyEnvelope {
    /// Human-readable name of the contained modality. Used for
    /// telemetry and error messages.
    pub fn modality_name(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Tabular(_) => "tabular",
            Self::Image(_) => "image",
            Self::Audio(_) => "audio",
        }
    }

    /// Borrow the underlying shared codec handle without committing
    /// to a modality.
    pub fn handle(&self) -> &SharedHandle {
        match self {
            Self::Text(e) => &e.handle,
            Self::Tabular(e) => &e.handle,
            Self::Image(e) => &e.handle,
            Self::Audio(e) => &e.handle,
        }
    }

    /// Clone the envelope's audit as a modality-erased [`AnyAudit`].
    /// Used by the engine's run-summary collector so it doesn't have
    /// to match on every variant.
    pub fn audit_cloned(&self) -> AnyAudit {
        match self {
            Self::Text(e) => e.document.audit.clone().into(),
            Self::Tabular(e) => e.document.audit.clone().into(),
            Self::Image(e) => e.document.audit.clone().into(),
            Self::Audio(e) => e.document.audit.clone().into(),
        }
    }
}

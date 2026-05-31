//! Per-document state accumulated across pipeline operations.
//!
//! A [`DocumentEnvelope<M>`] is created at import and travels through
//! every operation in the pipeline. Rich sources (PDFs with both
//! text and image layers) produce one root `DocumentEnvelope<Text>`
//! whose document hosts nested image/tabular documents via
//! [`TextBlock::Embed`]; the shared codec [`DocumentHandle`] on the
//! root envelope services reads and mutations for every nested doc.
//!
//! Each stage reads from and writes to the envelope until the
//! document is fully redacted.
//!
//! [`TextBlock::Embed`]: nvisy_ontology::modality::TextBlock::Embed

mod accessors;
mod any;
mod policy_store;
mod shared_data;
pub mod value_at;

use std::fmt;
use std::sync::Arc;

use nvisy_codec::DocumentHandle;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentMetadata, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::document::Document;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Modality;
use tokio::sync::Mutex;
use uuid::Uuid;

pub use self::any::AnyEnvelope;
pub(crate) use self::policy_store::Decision;
pub use self::policy_store::PolicyStore;
pub use self::shared_data::SharedData;

/// Shared codec handle across typed envelopes spawned from the same
/// source. Wrapped in `Arc<Mutex<_>>` because handle redaction methods
/// take `&mut self`; multiple modality-typed envelopes coordinate
/// reads and mutations through the lock.
pub type SharedHandle = Arc<Mutex<DocumentHandle>>;

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
    /// [`EntityRecord`]: nvisy_ontology::provenance::EntityRecord
    pub fn add_entities(&mut self, entities: impl IntoIterator<Item = Entity<M>>) {
        for entity in entities {
            self.document.audit.push_entity(entity);
        }
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

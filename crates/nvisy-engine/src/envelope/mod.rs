//! Per-document state accumulated across pipeline operations.
//!
//! A [`DocumentEnvelope<M>`] is created at import and travels through
//! every operation in the pipeline for one modality. Rich sources
//! (PDFs with both text and image layers) fan out into multiple
//! envelopes — one per modality — that share the same codec
//! [`DocumentHandle`] via `Arc`.
//!
//! Each stage reads from and writes to the envelope until the
//! document is fully redacted.
//!
//! [`shared`]: DocumentEnvelope::shared

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
use nvisy_ontology::entity::{Annotation, Entity};
use nvisy_ontology::modality::Modality;
use nvisy_ontology::provenance::{Audit, RedactionMap};
use tokio::sync::Mutex;
use uuid::Uuid;

pub use self::any::AnyEnvelope;
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
    /// tabular). Populated at import (native) or by extraction
    /// (OCR/STT/VLM).
    pub document: Option<Document<M>>,

    /// User-supplied annotations (inclusions, exclusions, labels)
    /// attached at upload time. Set during import from content
    /// metadata; `Inclusion` variants are also projected into
    /// [`Self::audit::entities`] so detection/redaction see them as
    /// pre-detected entities, while the original list stays here
    /// for exclusion filtering and label-driven policy scoping.
    pub annotations: Vec<Annotation<M>>,

    /// IDs of reference-data [`Context`]s loaded by [`LoadContext`]
    /// nodes. Each UUID resolves through the engine's context cache.
    ///
    /// [`Context`]: nvisy_ontology::context::Context
    /// [`LoadContext`]: crate::ingestion::LoadContext
    pub contexts: Vec<Uuid>,

    /// Per-document audit trail: entities, processing log, and
    /// redaction records.
    pub audit: Audit<M>,

    /// Mapping of entity IDs to original and replacement values.
    /// Populated during redaction. Not included in the public audit
    /// response, stored separately under access control.
    pub redaction_map: RedactionMap<M>,

    /// Run-wide shared state (policies, registry, key provider).
    /// Cheaply cloneable (`Arc`): all envelopes in a run share the
    /// same underlying data.
    pub shared: Arc<SharedData>,
}

impl<M: Modality> DocumentEnvelope<M> {
    /// Create a new envelope from a shared codec handle and metadata.
    pub async fn new(
        handle: SharedHandle,
        metadata: ContentMetadata,
        shared: Arc<SharedData>,
    ) -> Self {
        let source = handle.lock().await.source();
        let audit = Audit::new(source);
        Self {
            handle,
            metadata,
            document: None,
            annotations: Vec::new(),
            contexts: Vec::new(),
            audit,
            redaction_map: RedactionMap::new(),
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
        self.audit.entities.len()
    }

    /// Add detected entities, assigning sensitivity from entity kind.
    pub fn add_entities(&mut self, entities: impl IntoIterator<Item = Entity<M>>) {
        for mut entity in entities {
            if entity.sensitivity.is_none() {
                entity.sensitivity = Some(entity.entity_kind.sensitivity());
            }
            self.audit.entities.push(entity);
        }
    }
}

impl<M: Modality> fmt::Debug for DocumentEnvelope<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocumentEnvelope")
            .field("entities", &self.audit.entities.len())
            .field("contexts", &self.contexts.len())
            .field("entries", &self.audit.entries.len())
            .finish_non_exhaustive()
    }
}

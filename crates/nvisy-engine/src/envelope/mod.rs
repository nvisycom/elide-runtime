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
mod shared_data;
pub mod value_at;

use std::fmt;
use std::sync::Arc;

use nvisy_codec::DocumentHandle;
use nvisy_codec::handler::TextData;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentMetadata, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::context::Contexts;
use nvisy_ontology::document::Document;
use nvisy_ontology::entity::{Annotation, Entity};
use nvisy_ontology::modality::{Audio, Image, Modality, Tabular, Text};
use nvisy_ontology::provenance::{Audit, RedactionMap};
use tokio::sync::Mutex;

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
    /// metadata.
    ///
    /// Currently unused while annotation-driven inclusion seeding
    /// and exclusion filtering are reinstated on the typed envelope.
    pub annotations: Vec<Annotation<M>>,

    /// Reference-data contexts loaded by [`LoadContext`] nodes.
    ///
    /// [`LoadContext`]: crate::ingestion::LoadContext
    pub contexts: Contexts,

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
            contexts: Contexts::new(),
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

/// Text-specific accessors that need the codec handle.
impl DocumentEnvelope<Text> {
    /// Resolve a [`Text`] location to its text representation by
    /// reading from the codec handle.
    pub async fn value_at(&self, location: &Text) -> Option<String> {
        self.handle
            .lock()
            .await
            .read_text(location)
            .await
            .map(TextData::into_inner)
    }
}

/// Tabular-specific accessors that need the codec handle.
impl DocumentEnvelope<Tabular> {
    /// Resolve a [`Tabular`] location to its cell text by reading
    /// from the codec handle.
    pub async fn value_at(&self, location: &Tabular) -> Option<String> {
        self.handle
            .lock()
            .await
            .read_tabular(location)
            .await
            .map(TextData::into_inner)
    }
}

/// Image-specific accessors that look up text via the OCR document.
impl DocumentEnvelope<Image> {
    /// Resolve an [`Image`] location to its OCR'd text by exact
    /// bounding-box equality against the extraction document.
    pub async fn value_at(&self, location: &Image) -> Option<String> {
        let doc = self.document.as_ref()?;
        for block in &doc.blocks {
            if let Some(text) = lookup_image_block(&block.kind, location) {
                return Some(text);
            }
        }
        None
    }
}

/// Audio-specific accessors that look up text via the STT document.
impl DocumentEnvelope<Audio> {
    /// Resolve an [`Audio`] location to its transcribed text by exact
    /// time-span equality against the extraction document.
    pub async fn value_at(&self, location: &Audio) -> Option<String> {
        let doc = self.document.as_ref()?;
        for block in &doc.blocks {
            if let Some(text) = lookup_audio_block(&block.kind, location) {
                return Some(text);
            }
        }
        None
    }
}

fn lookup_image_block(
    kind: &nvisy_ontology::modality::ImageBlock,
    location: &Image,
) -> Option<String> {
    use nvisy_ontology::modality::ImageBlock;
    let (text, spans, region) = match kind {
        ImageBlock::Text {
            text,
            spans,
            region,
        }
        | ImageBlock::Heading {
            text,
            spans,
            region,
        }
        | ImageBlock::Table {
            text,
            spans,
            region,
        } => (text, spans, region),
        _ => return None,
    };
    if region == location {
        return Some(text.clone());
    }
    spans
        .iter()
        .find(|s| s.source == *location)
        .map(|s| text[s.text_start..s.text_end].to_owned())
}

fn lookup_audio_block(
    kind: &nvisy_ontology::modality::AudioBlock,
    location: &Audio,
) -> Option<String> {
    use nvisy_ontology::modality::AudioBlock;
    let AudioBlock::Speech {
        text,
        spans,
        time_span,
        ..
    } = kind
    else {
        return None;
    };
    if time_span == &location.time_span {
        return Some(text.clone());
    }
    spans
        .iter()
        .find(|s| s.source == *location)
        .map(|s| text[s.text_start..s.text_end].to_owned())
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

#[cfg(test)]
impl DocumentEnvelope<Text> {
    /// Create a test envelope from plain text content.
    pub(crate) async fn from_text(text: &str, shared: Arc<SharedData>) -> Self {
        let data = ContentData::from_text(ContentSource::new(), text);
        let meta = ContentMetadata::new().with_content_type("text/plain");
        let content = nvisy_core::content::Content::with_metadata(data, meta.clone());
        let handle = nvisy_formats::decode(&content).await.expect("decode text");
        Self::new(Arc::new(Mutex::new(handle)), meta, shared).await
    }
}

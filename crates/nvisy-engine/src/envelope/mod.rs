//! Per-document state accumulated across pipeline operations.
//!
//! A [`DocumentEnvelope`] is created at import and travels through
//! every operation in the pipeline. Each stage reads from and writes
//! to the envelope until the document is fully redacted.
//!
//! ```text
//! ContentData
//!   ↓ Import
//! DocumentEnvelope { handle, metadata, audit, … }
//!   ↓ OCR / STT extraction
//! DocumentEnvelope { handle, image|audio: Some(Document<M>), … }
//!   ↓ NER / CV / PatternMatch detection
//! DocumentEnvelope { …, audit { entities: Vec<Entity<AnyModality>>, … } }
//!   ↓ Deduplication / Ensemble
//! DocumentEnvelope { …, audit { entities (merged), … } }
//!   ↓ Policy Evaluation + Redaction
//! DocumentEnvelope { handle (redacted), audit { entities, entries, … } }
//! ```
//!
//! Each operation receives `&mut DocumentEnvelope` and reads/writes
//! fields directly. Run-wide shared state (policies, registry, key
//! provider) is available via the [`shared`] field.
//!
//! [`shared`]: DocumentEnvelope::shared

mod accessors;
mod shared_data;

use std::fmt;
use std::sync::Arc;

use nvisy_codec::DocumentHandle;
use nvisy_codec::handler::TextData;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentMetadata, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::context::Contexts;
use nvisy_ontology::document::Document;
use nvisy_ontology::entity::{Annotations, Entity};
use nvisy_ontology::modality::{AnyModality, Audio, Image, Tabular, Text};
use nvisy_ontology::provenance::{Audit, RedactionMap};

pub(crate) use self::shared_data::SharedData;

/// Per-document state that flows through the entire pipeline.
///
/// Owns the codec [`DocumentHandle`] (the canonical bytes — modified
/// in-place during redaction), per-modality [`Document<M>`] extraction
/// outputs (populated by OCR / STT when relevant), user annotations,
/// audit, and run-wide shared state.
///
/// Detected entities live on [`audit.entities`] as
/// `Vec<Entity<AnyModality>>` — recognizers lift their typed
/// `Entity<M>` outputs via [`Entity::erase`] at the audit boundary.
///
/// [`audit.entities`]: nvisy_ontology::provenance::Audit::entities
/// [`Entity::erase`]: nvisy_ontology::entity::Entity::erase
pub struct DocumentEnvelope {
    /// The codec handle for the document's bytes. Modified in-place
    /// during the redaction stage; encoded back to output bytes at
    /// the end.
    pub handle: DocumentHandle,

    /// Content metadata (MIME type, filename, etc.) from the
    /// original upload.
    pub metadata: ContentMetadata,

    /// Native or extracted text-modality representation. `None` for
    /// sources whose text representation hasn't been built yet (e.g.
    /// image documents before any text-layer pass).
    pub text: Option<Document<Text>>,

    /// Image-modality representation. Populated by OCR; `None` for
    /// non-image sources or when OCR hasn't run.
    pub image: Option<Document<Image>>,

    /// Audio-modality representation. Populated by STT; `None` for
    /// non-audio sources or when STT hasn't run.
    pub audio: Option<Document<Audio>>,

    /// Tabular-modality representation. Populated for tabular
    /// sources during import (in a follow-up — currently `None`).
    pub tabular: Option<Document<Tabular>>,

    /// User-supplied annotations (inclusions, exclusions, labels)
    /// attached at upload time. Set during import from content
    /// metadata.
    pub(crate) annotations: Annotations<AnyModality>,

    /// Reference-data contexts loaded by [`LoadContext`] nodes.
    ///
    /// [`LoadContext`]: crate::ingestion::LoadContext
    pub contexts: Contexts,

    /// Per-document audit trail: entities, processing log, and
    /// redaction records.
    pub audit: Audit,

    /// Mapping of entity IDs to original and replacement values.
    /// Populated during redaction. Not included in the public audit
    /// response, stored separately under access control.
    pub redaction_map: RedactionMap,

    /// Run-wide shared state (policies, registry, key provider).
    /// Cheaply cloneable (`Arc`): all envelopes in a run share the
    /// same underlying data.
    pub shared: Arc<SharedData>,
}

impl DocumentEnvelope {
    /// Create a new envelope from a content handle and metadata.
    pub fn new(handle: DocumentHandle, metadata: ContentMetadata, shared: Arc<SharedData>) -> Self {
        let audit = Audit::new(handle.source());
        Self {
            handle,
            metadata,
            text: None,
            image: None,
            audio: None,
            tabular: None,
            annotations: Annotations::new(),
            contexts: Contexts::new(),
            audit,
            redaction_map: RedactionMap::new(),
            shared,
        }
    }

    /// The document type of the underlying content.
    pub fn document_type(&self) -> DocumentType {
        self.handle.document_type()
    }

    /// Content source identity and lineage.
    pub fn source(&self) -> ContentSource {
        self.handle.source()
    }

    /// Encode the codec handle back to raw bytes.
    pub fn encode(&self) -> Result<ContentData, Error> {
        self.handle.encode()
    }

    /// Number of detected entities.
    pub fn entity_count(&self) -> usize {
        self.audit.entities.len()
    }

    /// Add detected entities, assigning sensitivity from entity kind
    /// and filtering out any that fall within exclusion annotations.
    ///
    /// Recognizers produce `Entity<M>` for a specific modality; the
    /// caller lifts each entity via [`Entity::erase`] before handing
    /// it off here. `Vec<Entity<M>>` consumers can do
    /// `entities.into_iter().map(Entity::erase)` inline.
    ///
    /// [`Entity::erase`]: nvisy_ontology::entity::Entity::erase
    pub async fn add_entities(&mut self, entities: impl IntoIterator<Item = Entity<AnyModality>>) {
        for mut entity in entities {
            if entity.sensitivity.is_none() {
                entity.sensitivity = Some(entity.entity_kind.sensitivity());
            }
            if self.annotations.is_empty() {
                self.audit.entities.push(entity);
            } else {
                let value = self.value_at(&entity.location).await;
                if !self.annotations.is_excluded(&entity, value.as_deref()) {
                    self.audit.entities.push(entity);
                }
            }
        }
    }

    /// Resolve a [`AnyModality`] location to its text representation,
    /// dispatching by modality.
    ///
    /// - **Text / Tabular**: read from the codec handle directly.
    /// - **Image**: look up by exact bounding-box equality in the
    ///   OCR [`Document<Image>`] populated by extraction; returns
    ///   `None` if OCR hasn't run.
    /// - **Audio**: look up by exact time-span equality in the STT
    ///   [`Document<Audio>`]; returns `None` if STT hasn't run.
    pub async fn value_at(&self, location: &AnyModality) -> Option<String> {
        match location {
            AnyModality::Text(loc) => self.handle.read_text(loc).await.map(TextData::into_inner),
            AnyModality::Tabular(loc) => self
                .handle
                .read_tabular(loc)
                .await
                .map(TextData::into_inner),
            AnyModality::Image(loc) => {
                let doc = self.image.as_ref()?;
                doc.spans()
                    .find(|(_, s)| s.source == *loc)
                    .map(|(b, s)| b.text[s.text_start..s.text_end].to_owned())
            }
            AnyModality::Audio(loc) => {
                let doc = self.audio.as_ref()?;
                doc.spans()
                    .find(|(_, s)| s.source == *loc)
                    .map(|(b, s)| b.text[s.text_start..s.text_end].to_owned())
            }
            _ => None,
        }
    }
}

impl fmt::Debug for DocumentEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocumentEnvelope")
            .field("document_type", &self.document_type())
            .field("source", &self.source())
            .field("entities", &self.audit.entities.len())
            .field("contexts", &self.contexts.len())
            .field("entries", &self.audit.entries.len())
            .finish()
    }
}

#[cfg(test)]
impl DocumentEnvelope {
    /// Create a test envelope from plain text content.
    pub(crate) async fn from_text(text: &str, shared: Arc<SharedData>) -> Self {
        let data = ContentData::from_text(ContentSource::new(), text);
        let meta = ContentMetadata::new().with_content_type("text/plain");
        let content = nvisy_core::content::Content::with_metadata(data, meta.clone());
        let handle = nvisy_formats::decode(&content).await.expect("decode text");
        Self::new(handle, meta, shared)
    }
}

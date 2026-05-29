//! File import operation.
//!
//! Decodes raw content into one or more [`AnyEnvelope`]s, optionally
//! applying pre-processing in order:
//!
//! 1. **Decompression** — decompress raw bytes (if format specified)
//! 2. **Decryption** — decrypt content (if encryption config specified)
//! 3. **Decode** — detect format and decode into a typed
//!    [`DocumentHandle`]
//! 4. **Dispatch** — wrap the handle in the matching
//!    [`DocumentEnvelope<M>`]; rich documents (PDF, DOCX) fan out
//!    into a `Text` and an `Image` envelope sharing one
//!    `Arc<Mutex<DocumentHandle>>`.
//! 5. **Seed** — convert any [`Inclusion`] annotations from the
//!    content metadata into pre-detected entities on the envelope's
//!    audit, and store the full annotation list on the envelope for
//!    downstream exclusion filtering.
//!
//! [`DocumentHandle`]: nvisy_codec::DocumentHandle
//! [`Inclusion`]: nvisy_ontology::entity::AnnotationKind::Inclusion

use std::mem;
use std::sync::Arc;

use nvisy_codec::HandleModality;
use nvisy_core::Result;
use nvisy_core::content::{Content, ContentData, ContentMetadata};
use nvisy_ontology::entity::{Annotation, ModelKind, ModelProvenance, inclusion_entities};
use nvisy_ontology::modality::{
    Audio, AudioExtraction, AudioMetadata, Image, ImageExtraction, ImageMetadata, Tabular,
    TabularExtraction, TabularMetadata, Text, TextExtraction, TextMetadata,
};
use tokio::sync::Mutex;

use crate::envelope::{AnyEnvelope, DocumentEnvelope, SharedData, SharedHandle};
use crate::ingestion::compression::CompressionService;
use crate::ingestion::encryption::{CryptoService, EncryptedContent};
use crate::ingestion::{CompressionAlgorithm, EncryptionAlgorithm, EncryptionConfig};

const TARGET: &str = "nvisy_engine::op::import_file";

/// Decodes raw content into one or more [`AnyEnvelope`]s, optionally
/// applying decompression and decryption beforehand.
#[derive(Default)]
pub struct Importer {
    decompression: Option<CompressionAlgorithm>,
    decryption: Option<EncryptionConfig>,
}

impl Importer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_decompression(mut self, format: Option<CompressionAlgorithm>) -> Self {
        self.decompression = format;
        self
    }

    pub fn with_decryption(mut self, config: Option<EncryptionConfig>) -> Self {
        self.decryption = config;
        self
    }

    pub async fn import(
        &self,
        content: Content,
        shared: &Arc<SharedData>,
    ) -> Result<Vec<AnyEnvelope>> {
        let mut content = content;

        if let Some(algorithm) = self.decompression {
            tracing::debug!(target: TARGET, ?algorithm, "decompressing content");
            let decompressed = CompressionService::new(algorithm).decompress(content.as_bytes())?;
            let source = content.content_source();
            content = replace_data(content, ContentData::new(source, decompressed));
        }

        if let Some(ref enc_cfg) = self.decryption {
            tracing::debug!(target: TARGET, key_id = %enc_cfg.key_id, "decrypting content");
            let crypto = CryptoService::new(&enc_cfg.key_id, shared.key_provider.clone());
            let encrypted = EncryptedContent {
                source: content.content_source(),
                ciphertext: bytes::Bytes::copy_from_slice(content.as_bytes()),
                key_id: enc_cfg.key_id.clone(),
                algorithm: EncryptionAlgorithm::Aes256Gcm,
            };
            let decrypted_data = crypto.decrypt(encrypted).await?;
            content = replace_data(content, decrypted_data);
        }

        let doc = nvisy_formats::decode(&content).await?;
        tracing::debug!(target: TARGET, doc_type = %doc.document_type(), "decoded document");
        let mut metadata = content.into_parts().1.unwrap_or_default();
        let annotations = mem::take(&mut metadata.annotations);

        let handle: SharedHandle = Arc::new(Mutex::new(doc));
        let envelopes = dispatch(handle, metadata, annotations, shared).await;
        tracing::debug!(
            target: TARGET,
            count = envelopes.len(),
            modalities = ?envelopes.iter().map(AnyEnvelope::modality_name).collect::<Vec<_>>(),
            "produced envelopes"
        );
        Ok(envelopes)
    }
}

/// Build the per-modality envelope(s) from a shared codec handle.
///
/// `annotations` is consumed: it's only applied to the `Text`
/// envelope today (the metadata field is `Vec<Annotation<Text>>`).
/// Non-text variants get an empty annotation list — when
/// `ContentMetadata` grows per-modality annotation buckets, route
/// each bucket to the matching branch here.
async fn dispatch(
    handle: SharedHandle,
    metadata: ContentMetadata,
    annotations: Vec<Annotation<Text>>,
    shared: &Arc<SharedData>,
) -> Vec<AnyEnvelope> {
    let (modality, has_header) = {
        let guard = handle.lock().await;
        (guard.modality(), guard.tabular_has_header())
    };
    match modality {
        HandleModality::Text => {
            let doc_meta = text_metadata_for(TextExtraction::Native);
            let env = build_text_envelope(handle, metadata, doc_meta, annotations, shared).await;
            vec![AnyEnvelope::Text(env)]
        }
        HandleModality::Tabular => {
            let doc_meta = tabular_metadata_for(tabular_extraction_from_header(has_header));
            let env =
                <DocumentEnvelope<Tabular>>::new(handle, metadata, doc_meta, Arc::clone(shared))
                    .await;
            vec![AnyEnvelope::Tabular(env)]
        }
        HandleModality::Image => {
            let doc_meta = image_metadata_for(ImageExtraction::Ocr(pending_provenance()));
            let env =
                <DocumentEnvelope<Image>>::new(handle, metadata, doc_meta, Arc::clone(shared))
                    .await;
            vec![AnyEnvelope::Image(env)]
        }
        HandleModality::Audio => {
            let doc_meta = audio_metadata_for(AudioExtraction::Transcription(pending_provenance()));
            let env =
                <DocumentEnvelope<Audio>>::new(handle, metadata, doc_meta, Arc::clone(shared))
                    .await;
            vec![AnyEnvelope::Audio(env)]
        }
        HandleModality::Rich => {
            // PDF/DOCX: fan out into Text + Image envelopes sharing
            // the same underlying handle so reads and mutations
            // stay coordinated under the codec's mutex.
            //
            // Text envelope is `Native` because today the rich-text
            // handler always reads the embedded text layer. The
            // `Recognized` path lights up when an image-only-PDF
            // OCR fallback lands.
            let text_meta = text_metadata_for(TextExtraction::Native);
            let image_meta = image_metadata_for(ImageExtraction::Ocr(pending_provenance()));
            let text_env = build_text_envelope(
                Arc::clone(&handle),
                metadata.clone(),
                text_meta,
                annotations,
                shared,
            )
            .await;
            let image_env =
                <DocumentEnvelope<Image>>::new(handle, metadata, image_meta, Arc::clone(shared))
                    .await;
            vec![AnyEnvelope::Text(text_env), AnyEnvelope::Image(image_env)]
        }
    }
}

async fn build_text_envelope(
    handle: SharedHandle,
    metadata: ContentMetadata,
    document_meta: TextMetadata,
    annotations: Vec<Annotation<Text>>,
    shared: &Arc<SharedData>,
) -> DocumentEnvelope<Text> {
    let mut envelope =
        <DocumentEnvelope<Text>>::new(handle, metadata, document_meta, Arc::clone(shared)).await;
    let seeded = inclusion_entities::<Text>(&annotations);
    if !seeded.is_empty() {
        tracing::debug!(
            target: TARGET,
            count = seeded.len(),
            "seeding inclusion annotations as entities"
        );
        envelope.add_entities(seeded);
    }
    envelope.document.annotations = annotations;
    envelope
}

fn text_metadata_for(extraction: TextExtraction) -> TextMetadata {
    TextMetadata {
        extraction,
        languages: Vec::new(),
    }
}

fn tabular_metadata_for(extraction: TabularExtraction) -> TabularMetadata {
    TabularMetadata {
        extraction,
        headers: Vec::new(),
        sheet_names: Vec::new(),
    }
}

fn image_metadata_for(extraction: ImageExtraction) -> ImageMetadata {
    ImageMetadata {
        extraction,
        languages: Vec::new(),
        pages: Vec::new(),
    }
}

fn audio_metadata_for(extraction: AudioExtraction) -> AudioMetadata {
    AudioMetadata {
        extraction,
        languages: Vec::new(),
        sample_rate_hz: None,
        channels: None,
    }
}

/// Map the codec's `has_header()` signal to a [`TabularExtraction`].
///
/// `None` would mean the handle isn't tabular and shouldn't reach
/// this site; we still pick a safe default (`SchemaInferred`) so the
/// code path is total.
fn tabular_extraction_from_header(has_header: Option<bool>) -> TabularExtraction {
    match has_header {
        Some(true) => TabularExtraction::SchemaTyped,
        Some(false) | None => TabularExtraction::SchemaInferred,
    }
}

/// Placeholder [`ModelProvenance`] stamped at import time on
/// envelopes whose real provenance is only known after extraction
/// runs (OCR, STT).
///
/// `OcrExtractor::run` and `SttExtractor::run` overwrite the
/// containing [`ImageExtraction::Ocr`] / [`AudioExtraction::Transcription`]
/// variant with the actual backend provenance.
fn pending_provenance() -> ModelProvenance {
    ModelProvenance::new("pending", ModelKind::SelfHosted)
}

/// Replace the data payload of a [`Content`] while preserving its metadata.
fn replace_data(content: Content, data: ContentData) -> Content {
    match content.into_parts().1 {
        Some(meta) => Content::with_metadata(data, meta),
        None => Content::new(data),
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::content::{Content, ContentData, ContentMetadata};
    use nvisy_ontology::entity::{
        AnnotationKind, AnnotationTarget, EntityCategory, EntityKind, RecognitionMethod,
    };

    use super::*;
    use crate::envelope::SharedData;

    fn shared() -> Arc<SharedData> {
        let dir = tempfile::tempdir().unwrap();
        let registry = crate::ingestion::registry::Registry::open(dir.path()).unwrap();
        SharedData::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), registry)
    }

    fn text_content(text: &str, annotations: Vec<Annotation<Text>>) -> Content {
        let meta = ContentMetadata::new()
            .with_content_type("text/plain")
            .with_annotations(annotations);
        Content::with_metadata(ContentData::from(text.to_owned()), meta)
    }

    #[tokio::test]
    async fn unknown_format_errors() {
        let shared = shared();
        let content = Content::new(ContentData::from("plain text has no magic bytes"));
        assert!(Importer::new().import(content, &shared).await.is_err());
    }

    #[tokio::test]
    async fn text_import_yields_single_text_envelope() {
        let shared = shared();
        let content = text_content("Hello, world!", Vec::new());
        let envelopes = Importer::new().import(content, &shared).await.unwrap();
        assert_eq!(envelopes.len(), 1);
        assert!(envelopes[0].is_text());
    }

    #[tokio::test]
    async fn text_import_seeds_inclusion_annotations_as_entities() {
        let shared = shared();
        let annotation = Annotation {
            name: Some("uploader".into()),
            kind: AnnotationKind::Inclusion {
                category: EntityCategory::PersonalIdentity,
                entity_kind: EntityKind::PersonName,
                target: AnnotationTarget::Value("Jane Doe".into()),
                confidence: None,
            },
        };
        let content = text_content("Jane Doe lives somewhere.", vec![annotation.clone()]);

        let envelopes = Importer::new().import(content, &shared).await.unwrap();
        let AnyEnvelope::Text(env) = envelopes.into_iter().next().unwrap() else {
            panic!("expected a Text envelope");
        };

        assert_eq!(env.document.audit.records.len(), 1);
        let entity = &env.document.audit.records[0].entity;
        assert_eq!(entity.entity_kind, EntityKind::PersonName);
        assert!(matches!(
            entity.recognition_methods.first(),
            Some(RecognitionMethod::Annotation(_))
        ));
        // Annotations are retained on the document for downstream
        // exclusion filtering and label-driven policy scoping.
        assert_eq!(env.document.annotations, vec![annotation]);
    }

    #[tokio::test]
    async fn tabular_import_yields_single_tabular_envelope() {
        let shared = shared();
        let meta = ContentMetadata::new().with_content_type("text/csv");
        let data = ContentData::from("name,age\nAlice,30\nBob,40\n".to_owned());
        let content = Content::with_metadata(data, meta);

        let envelopes = Importer::new().import(content, &shared).await.unwrap();
        assert_eq!(envelopes.len(), 1);
        assert!(envelopes[0].is_tabular());
    }
}

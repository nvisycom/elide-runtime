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
use nvisy_core::content::{AnyAnnotations, Content, ContentData, ContentMetadata};
use nvisy_ontology::entity::{Annotation, AnnotationKind, AnnotationTarget, LabelAnnotation};
use nvisy_ontology::modality::{
    Audio, AudioExtraction, AudioMetadata, Image, ImageExtraction, ImageMetadata, Modality,
    Tabular, TabularExtraction, TabularMetadata, Text, TextExtraction, TextMetadata,
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
/// Each modality-typed [`Annotation<M>`] bucket from
/// [`AnyAnnotations`] is routed to the envelope of the matching
/// modality. Document-level labels are modality-agnostic and clone
/// into every envelope spawned from the source so policy rules /
/// detector prompts can read them uniformly.
///
/// Assertion-strength inclusions are *not* materialised into
/// entities here — that requires the document's blocks to be
/// populated. The extraction stage performs the seeding for every
/// modality once blocks exist.
async fn dispatch(
    handle: SharedHandle,
    metadata: ContentMetadata,
    mut annotations: AnyAnnotations,
    shared: &Arc<SharedData>,
) -> Vec<AnyEnvelope> {
    let (modality, has_header) = {
        let guard = handle.lock().await;
        (guard.modality(), guard.tabular_has_header())
    };
    match modality {
        HandleModality::Text => {
            let doc_meta = TextMetadata::from(TextExtraction::Native);
            let mut env =
                <DocumentEnvelope<Text>>::new(handle, metadata, doc_meta, Arc::clone(shared)).await;
            attach_annotations(
                &mut env,
                mem::take(&mut annotations.text),
                annotations.labels,
            );
            vec![AnyEnvelope::Text(env)]
        }
        HandleModality::Tabular => {
            let doc_meta = TabularMetadata::from(TabularExtraction::from_header_signal(has_header));
            let mut env =
                <DocumentEnvelope<Tabular>>::new(handle, metadata, doc_meta, Arc::clone(shared))
                    .await;
            attach_annotations(
                &mut env,
                mem::take(&mut annotations.tabular),
                annotations.labels,
            );
            vec![AnyEnvelope::Tabular(env)]
        }
        HandleModality::Image => {
            let doc_meta = ImageMetadata::from(ImageExtraction::Pending);
            let mut env =
                <DocumentEnvelope<Image>>::new(handle, metadata, doc_meta, Arc::clone(shared))
                    .await;
            attach_annotations(
                &mut env,
                mem::take(&mut annotations.image),
                annotations.labels,
            );
            vec![AnyEnvelope::Image(env)]
        }
        HandleModality::Audio => {
            let doc_meta = AudioMetadata::from(AudioExtraction::Pending);
            let mut env =
                <DocumentEnvelope<Audio>>::new(handle, metadata, doc_meta, Arc::clone(shared))
                    .await;
            attach_annotations(
                &mut env,
                mem::take(&mut annotations.audio),
                annotations.labels,
            );
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
            let text_meta = TextMetadata::from(TextExtraction::Native);
            let image_meta = ImageMetadata::from(ImageExtraction::Pending);
            let mut text_env = <DocumentEnvelope<Text>>::new(
                Arc::clone(&handle),
                metadata.clone(),
                text_meta,
                Arc::clone(shared),
            )
            .await;
            attach_annotations(
                &mut text_env,
                mem::take(&mut annotations.text),
                annotations.labels.clone(),
            );
            let mut image_env =
                <DocumentEnvelope<Image>>::new(handle, metadata, image_meta, Arc::clone(shared))
                    .await;
            attach_annotations(
                &mut image_env,
                mem::take(&mut annotations.image),
                annotations.labels,
            );
            vec![AnyEnvelope::Text(text_env), AnyEnvelope::Image(image_env)]
        }
    }
}

/// Store user annotations on the envelope and synthesize entities
/// for every [`Assert`]-strength inclusion whose target carries a
/// concrete [`AnnotationTarget::Location`]. `Value` inclusions are
/// left on `envelope.document.annotations` for downstream detectors
/// to consume (the pattern recognizer materialises them as
/// user-defined patterns, then every occurrence gets detected the
/// normal way).
///
/// [`Assert`]: nvisy_ontology::entity::AnnotationStrength::Assert
/// [`AnnotationTarget::Location`]: nvisy_ontology::entity::AnnotationTarget::Location
fn attach_annotations<M: Modality>(
    envelope: &mut DocumentEnvelope<M>,
    annotations: Vec<Annotation<M>>,
    labels: Vec<LabelAnnotation>,
) {
    if !labels.is_empty() {
        tracing::debug!(
            target: TARGET,
            count = labels.len(),
            "attaching labels to envelope",
        );
    }
    if !annotations.is_empty() {
        tracing::debug!(
            target: TARGET,
            count = annotations.len(),
            "attaching annotations to envelope",
        );
        let mut synthesized = 0;
        for ann in &annotations {
            if let AnnotationKind::Inclusion {
                target: AnnotationTarget::Location(loc),
                ..
            } = &ann.kind
                && let Some(entity) = ann.to_inclusion_entity(loc.clone())
            {
                envelope.add_entities(std::iter::once(entity));
                synthesized += 1;
            }
        }
        if synthesized > 0 {
            tracing::debug!(
                target: TARGET,
                count = synthesized,
                "synthesized entities from Assert+Location inclusions",
            );
        }
    }
    envelope.document.annotations = annotations;
    envelope.document.labels = labels;
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
    use nvisy_core::content::{AnyAnnotations, Content, ContentData, ContentMetadata};
    use nvisy_ontology::entity::{
        Annotation, AnnotationKind, AnnotationStrength, AnnotationTarget, EntityCategory,
        EntityKind, LabelAnnotation, RecognitionMethod,
    };

    use super::*;
    use crate::envelope::SharedData;

    fn shared() -> Arc<SharedData> {
        let dir = tempfile::tempdir().unwrap();
        let registry = crate::ingestion::registry::Registry::open(dir.path()).unwrap();
        SharedData::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), registry)
    }

    fn text_content(text: &str, annotations: AnyAnnotations) -> Content {
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
        let content = text_content("Hello, world!", AnyAnnotations::default());
        let envelopes = Importer::new().import(content, &shared).await.unwrap();
        assert_eq!(envelopes.len(), 1);
        assert!(envelopes[0].is_text());
    }

    #[tokio::test]
    async fn assert_location_inclusion_synthesizes_entity_at_import() {
        let shared = shared();
        let annotation = Annotation {
            name: Some("uploader".into()),
            kind: AnnotationKind::Inclusion {
                category: Some(EntityCategory::PersonalIdentity),
                entity_kind: Some(EntityKind::PersonName),
                target: AnnotationTarget::Location(Text::new(0, 8)),
                strength: AnnotationStrength::Assert,
                confidence: None,
            },
        };
        let annotations = AnyAnnotations {
            text: vec![annotation.clone()],
            ..AnyAnnotations::default()
        };
        let content = text_content("Jane Doe lives somewhere.", annotations);

        let envelopes = Importer::new().import(content, &shared).await.unwrap();
        let AnyEnvelope::Text(env) = envelopes.into_iter().next().unwrap() else {
            panic!("expected a Text envelope");
        };

        assert_eq!(env.document.audit.records.len(), 1);
        let entity = &env.document.audit.records[0].entity;
        assert_eq!(entity.entity_kind, EntityKind::PersonName);
        assert_eq!(entity.location, Text::new(0, 8));
        assert!(matches!(
            entity.recognition_methods.first(),
            Some(RecognitionMethod::Annotation(_))
        ));
        // Annotations are retained on the document for downstream
        // consumers (pattern recognizer for Value targets, prompt
        // builders for hints, post-filter for exclusions).
        assert_eq!(env.document.annotations, vec![annotation]);
    }

    #[tokio::test]
    async fn assert_value_inclusion_is_not_synthesized_at_import() {
        // Value inclusions are handed to the pattern recognizer as
        // user-defined deny-list patterns; the importer leaves them
        // on the envelope for downstream consumption rather than
        // materialising a single first-occurrence entity.
        let shared = shared();
        let annotation = Annotation {
            name: None,
            kind: AnnotationKind::Inclusion {
                category: Some(EntityCategory::PersonalIdentity),
                entity_kind: Some(EntityKind::PersonName),
                target: AnnotationTarget::Value("Jane".into()),
                strength: AnnotationStrength::Assert,
                confidence: None,
            },
        };
        let annotations = AnyAnnotations {
            text: vec![annotation.clone()],
            ..AnyAnnotations::default()
        };
        let content = text_content("Jane and Jane.", annotations);

        let envelopes = Importer::new().import(content, &shared).await.unwrap();
        let AnyEnvelope::Text(env) = envelopes.into_iter().next().unwrap() else {
            panic!("expected a Text envelope");
        };

        assert_eq!(env.document.audit.records.len(), 0);
        assert_eq!(env.document.annotations, vec![annotation]);
    }

    #[tokio::test]
    async fn hint_inclusion_is_not_synthesized_at_import() {
        let shared = shared();
        let annotation = Annotation {
            name: None,
            kind: AnnotationKind::Inclusion {
                category: Some(EntityCategory::PersonalIdentity),
                entity_kind: Some(EntityKind::PersonName),
                target: AnnotationTarget::Location(Text::new(0, 4)),
                strength: AnnotationStrength::Hint,
                confidence: None,
            },
        };
        let annotations = AnyAnnotations {
            text: vec![annotation.clone()],
            ..AnyAnnotations::default()
        };
        let content = text_content("Jane lives here.", annotations);

        let envelopes = Importer::new().import(content, &shared).await.unwrap();
        let AnyEnvelope::Text(env) = envelopes.into_iter().next().unwrap() else {
            panic!("expected a Text envelope");
        };

        assert_eq!(env.document.audit.records.len(), 0);
        assert_eq!(env.document.annotations, vec![annotation]);
    }

    #[tokio::test]
    async fn labels_propagate_to_every_envelope() {
        let shared = shared();
        let annotations = AnyAnnotations {
            labels: vec![LabelAnnotation::new("medical")],
            ..AnyAnnotations::default()
        };
        let content = text_content("Hello, world!", annotations);
        let envelopes = Importer::new().import(content, &shared).await.unwrap();
        let AnyEnvelope::Text(env) = envelopes.into_iter().next().unwrap() else {
            panic!("expected a Text envelope");
        };
        assert_eq!(env.document.labels.len(), 1);
        assert_eq!(env.document.labels[0].label, "medical");
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

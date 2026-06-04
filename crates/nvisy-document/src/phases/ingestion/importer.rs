//! File import operation.
//!
//! Decodes raw content into one [`DocumentTree`], optionally applying
//! pre-processing in order:
//!
//! 1. **Decompression** — decompress raw bytes (if format specified)
//! 2. **Decryption** — decrypt content (if encryption config specified)
//! 3. **Decode** — detect format and decode into a typed
//!    [`DocumentHandle`]
//! 4. **Dispatch** — wrap the handle in the matching [`AnyDocument`]
//!    variant. Rich sources (PDF, DOCX) land as
//!    [`AnyDocument::Text`]; their image content surfaces as nested
//!    `Document<Image>` children inside [`TextBlock::Embed`] blocks
//!    once the extraction phase runs.
//! 5. **Seed** — convert any [`Inclusion`] annotations from the
//!    content metadata into pre-detected entities on the root
//!    document's audit, and store the full annotation list on the
//!    document for downstream exclusion filtering.
//!
//! [`DocumentHandle`]: nvisy_codec::DocumentHandle
//! [`AnyDocument`]: crate::core::AnyDocument
//! [`TextBlock::Embed`]: crate::modality::TextBlock::Embed
//! [`Inclusion`]: nvisy_core::entity::AnnotationKind::Inclusion

use std::mem;
use std::sync::Arc;

use nvisy_codec::HandleModality;
use nvisy_core::Result;
use nvisy_core::content::{AnyAnnotations, Content, ContentData, ContentMetadata};
use nvisy_core::entity::{Annotation, LabelAnnotation};
use nvisy_core::modality::{Audio, Image, Tabular, Text};
use nvisy_formats::decode;
use tokio::sync::Mutex;

use crate::core::{AnyDocument, DocumentTree, SharedData, SharedHandle};
use crate::document::Document;
use crate::modality::{
    AudioExtraction, AudioMetadata, DocumentModality, ImageExtraction, ImageMetadata,
    TabularExtraction, TabularMetadata, TextExtraction, TextMetadata,
};
use crate::phases::ingestion::compression::CompressionService;
use crate::phases::ingestion::encryption::{CryptoService, EncryptedContent};
use crate::phases::ingestion::{CompressionAlgorithm, EncryptionAlgorithm, EncryptionConfig};

const TARGET: &str = "nvisy_engine::op::import_file";

/// Decodes raw content into one [`DocumentTree`], optionally applying
/// decompression and decryption beforehand.
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

    /// Decode `content` into a single-element `Vec<DocumentTree>`.
    /// The vector shape is preserved from the previous `AnyEnvelope`
    /// fan-out so the orchestrator's collection loop stays uniform —
    /// even though every source path now yields exactly one tree.
    pub async fn import(
        &self,
        content: Content,
        shared: &Arc<SharedData>,
    ) -> Result<Vec<DocumentTree>> {
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

        let doc = decode(&content).await?;
        tracing::debug!(target: TARGET, doc_type = %doc.document_type(), "decoded document");
        let mut metadata = content.into_parts().1.unwrap_or_default();
        let annotations = mem::take(&mut metadata.annotations);

        let handle: SharedHandle = Arc::new(Mutex::new(doc));
        let tree = dispatch(handle, metadata, annotations).await;
        tracing::debug!(
            target: TARGET,
            modality = tree.root.modality_name(),
            "produced tree",
        );
        Ok(vec![tree])
    }
}

/// Build the per-modality root document and wrap it in a fresh tree.
async fn dispatch(
    handle: SharedHandle,
    metadata: ContentMetadata,
    mut annotations: AnyAnnotations,
) -> DocumentTree {
    let (modality, has_header) = {
        let guard = handle.lock().await;
        (guard.modality(), guard.tabular_has_header())
    };
    let source = handle.lock().await.source();
    let root = match modality {
        HandleModality::Text => {
            let mut doc = Document::<Text>::new(TextMetadata::from(TextExtraction::Native), source);
            attach_annotations(
                &mut doc,
                mem::take(&mut annotations.text),
                annotations.labels.clone(),
            );
            AnyDocument::Text(doc)
        }
        HandleModality::Tabular => {
            let mut doc = Document::<Tabular>::new(
                TabularMetadata::from(TabularExtraction::from_header_signal(has_header)),
                source,
            );
            attach_annotations(
                &mut doc,
                mem::take(&mut annotations.tabular),
                annotations.labels.clone(),
            );
            AnyDocument::Tabular(doc)
        }
        HandleModality::Image => {
            let mut doc =
                Document::<Image>::new(ImageMetadata::from(ImageExtraction::Pending), source);
            attach_annotations(
                &mut doc,
                mem::take(&mut annotations.image),
                annotations.labels.clone(),
            );
            AnyDocument::Image(doc)
        }
        HandleModality::Audio => {
            let mut doc =
                Document::<Audio>::new(AudioMetadata::from(AudioExtraction::Pending), source);
            attach_annotations(
                &mut doc,
                mem::take(&mut annotations.audio),
                annotations.labels.clone(),
            );
            AnyDocument::Audio(doc)
        }
        HandleModality::Rich => {
            // PDF/DOCX: one root Text doc. Image content surfaces
            // as nested `Document<Image>` children via
            // `TextBlock::Embed`, populated by the image extraction
            // step from the same shared codec handle.
            //
            // User-supplied image annotations on a Rich source are
            // dropped with a warn-log: under the nested model image
            // annotations target the nested image doc, which doesn't
            // exist until extraction runs.
            let dropped_image_annotations = annotations.image.len();
            if dropped_image_annotations > 0 {
                tracing::warn!(
                    target: TARGET,
                    count = dropped_image_annotations,
                    "dropping image annotations on rich source: nested-document seeding not implemented",
                );
            }
            let mut doc = Document::<Text>::new(TextMetadata::from(TextExtraction::Native), source);
            attach_annotations(
                &mut doc,
                mem::take(&mut annotations.text),
                annotations.labels.clone(),
            );
            AnyDocument::Text(doc)
        }
    };
    DocumentTree::new(root, handle, metadata)
}

/// Store user annotations on the document and synthesise entities
/// for every [`Assert`]-strength inclusion. [`Hint`]-strength
/// inclusions stay on the document for downstream prompt-builder
/// consumption; exclusions stay for the post-detection filter.
///
/// [`Assert`]: nvisy_core::entity::AnnotationStrength::Assert
/// [`Hint`]: nvisy_core::entity::AnnotationStrength::Hint
fn attach_annotations<M: DocumentModality>(
    doc: &mut Document<M>,
    annotations: Vec<Annotation<M>>,
    labels: Vec<LabelAnnotation>,
) {
    if !labels.is_empty() {
        tracing::debug!(
            target: TARGET,
            count = labels.len(),
            "attaching labels to document",
        );
    }
    if !annotations.is_empty() {
        tracing::debug!(
            target: TARGET,
            count = annotations.len(),
            "attaching annotations to document",
        );
        let mut synthesized = 0;
        for ann in &annotations {
            if let Some(entity) = ann.to_inclusion_entity() {
                doc.add_entities(std::iter::once(entity));
                synthesized += 1;
            }
        }
        if synthesized > 0 {
            tracing::debug!(
                target: TARGET,
                count = synthesized,
                "synthesized entities from Assert inclusions",
            );
        }
    }
    doc.annotations = annotations;
    doc.labels = labels;
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
    use nvisy_core::entity::{
        Annotation, AnnotationKind, AnnotationStrength, EntityKind, LabelAnnotation,
        TrailProvenance,
    };
    use nvisy_core::modality::TextLocation;

    use super::*;
    use crate::core::SharedData;
    use crate::phases::ingestion::registry::Registry;

    fn shared() -> Arc<SharedData> {
        let dir = tempfile::tempdir().unwrap();
        let registry = Registry::open(dir.path()).unwrap();
        SharedData::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), registry)
    }

    fn text_content(text: &str, annotations: AnyAnnotations) -> Content {
        let meta = ContentMetadata::new()
            .with_content_type("text/plain")
            .with_annotations(annotations);
        Content::with_metadata(ContentData::from(text.to_owned()), meta)
    }

    fn text_root(tree: DocumentTree) -> Document<Text> {
        match tree.root {
            AnyDocument::Text(doc) => doc,
            other => panic!("expected a Text root, got {}", other.modality_name()),
        }
    }

    #[tokio::test]
    async fn unknown_format_errors() {
        let shared = shared();
        let content = Content::new(ContentData::from("plain text has no magic bytes"));
        assert!(Importer::new().import(content, &shared).await.is_err());
    }

    #[tokio::test]
    async fn text_import_yields_single_text_tree() {
        let shared = shared();
        let content = text_content("Hello, world!", AnyAnnotations::default());
        let trees = Importer::new().import(content, &shared).await.unwrap();
        assert_eq!(trees.len(), 1);
        assert!(matches!(trees[0].root, AnyDocument::Text(_)));
    }

    #[tokio::test]
    async fn assert_inclusion_synthesizes_entity_at_import() {
        let shared = shared();
        let annotation = Annotation {
            name: Some("uploader".into()),
            kind: AnnotationKind::Inclusion {
                entity_kind: Some(EntityKind::PersonName),
                target: TextLocation::new(0, 8),
                strength: AnnotationStrength::Assert,
            },
        };
        let annotations = AnyAnnotations {
            text: vec![annotation.clone()],
            ..AnyAnnotations::default()
        };
        let content = text_content("Jane Doe lives somewhere.", annotations);

        let trees = Importer::new().import(content, &shared).await.unwrap();
        let doc = text_root(trees.into_iter().next().unwrap());

        assert_eq!(doc.audit.records.len(), 1);
        let entity = &doc.audit.records[0].entity;
        assert_eq!(entity.entity_kind, EntityKind::PersonName);
        assert_eq!(entity.location, TextLocation::new(0, 8));
        assert!(
            entity
                .trail
                .first()
                .is_some_and(|s| matches!(s.provenance, TrailProvenance::Annotation(_)))
        );
        assert_eq!(doc.annotations, vec![annotation]);
    }

    #[tokio::test]
    async fn hint_inclusion_is_not_synthesized_at_import() {
        let shared = shared();
        let annotation = Annotation {
            name: None,
            kind: AnnotationKind::Inclusion {
                entity_kind: Some(EntityKind::PersonName),
                target: TextLocation::new(0, 4),
                strength: AnnotationStrength::Hint { confidence: None },
            },
        };
        let annotations = AnyAnnotations {
            text: vec![annotation.clone()],
            ..AnyAnnotations::default()
        };
        let content = text_content("Jane lives here.", annotations);

        let trees = Importer::new().import(content, &shared).await.unwrap();
        let doc = text_root(trees.into_iter().next().unwrap());

        assert_eq!(doc.audit.records.len(), 0);
        assert_eq!(doc.annotations, vec![annotation]);
    }

    #[tokio::test]
    async fn labels_propagate_to_root_document() {
        let shared = shared();
        let annotations = AnyAnnotations {
            labels: vec![LabelAnnotation::new("medical")],
            ..AnyAnnotations::default()
        };
        let content = text_content("Hello, world!", annotations);
        let trees = Importer::new().import(content, &shared).await.unwrap();
        let doc = text_root(trees.into_iter().next().unwrap());
        assert_eq!(doc.labels.len(), 1);
        assert_eq!(doc.labels[0].label, "medical");
    }

    #[tokio::test]
    async fn tabular_import_yields_single_tabular_tree() {
        let shared = shared();
        let meta = ContentMetadata::new().with_content_type("text/csv");
        let data = ContentData::from("name,age\nAlice,30\nBob,40\n".to_owned());
        let content = Content::with_metadata(data, meta);

        let trees = Importer::new().import(content, &shared).await.unwrap();
        assert_eq!(trees.len(), 1);
        assert!(matches!(trees[0].root, AnyDocument::Tabular(_)));
    }
}

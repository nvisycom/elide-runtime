//! File import operation.
//!
//! Decodes raw content into one [`AnyTree`], optionally applying
//! pre-processing in order:
//!
//! 1. **Decompression** — decompress raw bytes (if format specified)
//! 2. **Decryption** — decrypt content (if encryption config specified)
//! 3. **Decode** — resolve a codec from the [`CodecRegistry`] by the
//!    content's extension or MIME hint, then decode into an
//!    [`UntypedDocumentHandle`].
//! 4. **Dispatch** — match the untyped handle once, build the matching
//!    typed [`DocumentTree<M>`], and wrap it in the [`AnyTree`]
//!    variant the orchestrator dispatches on.
//! 5. **Seed** — convert any [`Inclusion`] annotations from the
//!    content metadata into pre-detected entities on the root
//!    document's audit, and store the full annotation list on the
//!    document for downstream exclusion filtering.
//!
//! [`AnyTree`]: crate::core::AnyTree
//! [`DocumentTree<M>`]: crate::core::DocumentTree
//! [`UntypedDocumentHandle`]: nvisy_codec::UntypedDocumentHandle
//! [`CodecRegistry`]: nvisy_codec::CodecRegistry
//! [`Inclusion`]: nvisy_core::entity::AnnotationKind::Inclusion

use std::mem;
use std::sync::Arc;

use nvisy_codec::content::{Content, ContentData, ContentMetadata, ContentSource};
use nvisy_codec::{CodecRegistry, UntypedDocumentHandle};
use nvisy_core::entity::{Annotation, AnyAnnotations, LabelAnnotation};
use nvisy_core::modality::{Audio, Image, Tabular, Text};
use nvisy_core::{Error, Result};

use crate::core::{AnyTree, DocumentTree, SharedData};
use crate::document::Document;
use crate::modality::{
    AudioExtraction, AudioMetadata, DocumentModality, ImageExtraction, ImageMetadata,
    TabularExtraction, TabularMetadata, TextExtraction, TextMetadata,
};
use crate::phases::ingestion::compression::CompressionService;
use crate::phases::ingestion::encryption::{CryptoService, EncryptedContent};
use crate::phases::ingestion::{CompressionAlgorithm, EncryptionAlgorithm, EncryptionConfig};

const TARGET: &str = "nvisy_document::op::import_file";

/// Decodes raw content into one [`AnyTree`], optionally applying
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

    /// Decode `content` into a single-element `Vec<AnyTree>`. The
    /// vector shape is preserved so the orchestrator's collection
    /// loop stays uniform across import sources.
    pub async fn import(&self, content: Content, shared: &Arc<SharedData>) -> Result<Vec<AnyTree>> {
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

        let untyped = decode(&shared.codec_registry, &content).await?;
        tracing::debug!(
            target: TARGET,
            format = %untyped.format(),
            modality = ?untyped.modality(),
            "decoded document",
        );

        let (data, metadata) = content.into_parts();
        let mut metadata = metadata.unwrap_or_default();
        let annotations = mem::take(&mut metadata.annotations);
        let source = data.content_source;

        let tree = build_tree(untyped, source, metadata, annotations);
        tracing::debug!(
            target: TARGET,
            modality = tree.modality_name(),
            "produced tree",
        );
        Ok(vec![tree])
    }
}

/// Resolve a format from the registry by extension (preferred) or
/// MIME content type, then decode the raw bytes through its loader.
async fn decode(registry: &CodecRegistry, content: &Content) -> Result<UntypedDocumentHandle> {
    let format = content
        .file_extension()
        .and_then(|ext| registry.by_extension(ext))
        .or_else(|| {
            content
                .content_type()
                .and_then(|ct| registry.by_content_type(ct))
        })
        .ok_or_else(|| {
            Error::validation(
                format!(
                    "no codec registered for extension `{:?}` / content-type `{:?}`",
                    content.file_extension(),
                    content.content_type(),
                ),
                TARGET,
            )
        })?;
    format.loader.decode(content.data().clone()).await
}

/// Build the per-modality typed tree from an [`UntypedDocumentHandle`],
/// stamping the document's extraction provenance and seeding any
/// inclusion annotations onto the root document's audit.
fn build_tree(
    untyped: UntypedDocumentHandle,
    source: ContentSource,
    metadata: ContentMetadata,
    mut annotations: AnyAnnotations,
) -> AnyTree {
    match untyped {
        UntypedDocumentHandle::Text(handle) => {
            let mut doc = Document::<Text>::new(TextMetadata::from(TextExtraction::Native), source);
            attach_annotations(
                &mut doc,
                mem::take(&mut annotations.text),
                annotations.labels.clone(),
            );
            AnyTree::Text(DocumentTree::new(doc, handle, metadata))
        }
        UntypedDocumentHandle::Tabular(handle) => {
            let mut doc = Document::<Tabular>::new(
                TabularMetadata::from(TabularExtraction::SchemaInferred),
                source,
            );
            attach_annotations(
                &mut doc,
                mem::take(&mut annotations.tabular),
                annotations.labels.clone(),
            );
            AnyTree::Tabular(DocumentTree::new(doc, handle, metadata))
        }
        UntypedDocumentHandle::Image(handle) => {
            let mut doc =
                Document::<Image>::new(ImageMetadata::from(ImageExtraction::Pending), source);
            attach_annotations(
                &mut doc,
                mem::take(&mut annotations.image),
                annotations.labels.clone(),
            );
            AnyTree::Image(DocumentTree::new(doc, handle, metadata))
        }
        UntypedDocumentHandle::Audio(handle) => {
            let mut doc =
                Document::<Audio>::new(AudioMetadata::from(AudioExtraction::Pending), source);
            attach_annotations(
                &mut doc,
                mem::take(&mut annotations.audio),
                annotations.labels.clone(),
            );
            AnyTree::Audio(DocumentTree::new(doc, handle, metadata))
        }
    }
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

//! Unified redaction action -- applies text, image, tabular, and audio redactions.

mod text;
mod tabular;
mod image;
mod audio;

use std::collections::HashMap;
use uuid::Uuid;

use nvisy_codec::handler::{TxtHandler, CsvHandler, PngHandler, WavHandler};
use nvisy_codec::document::Document;
use nvisy_detection::{Entity, Redaction};
use nvisy_core::Error;

use text::apply_text_doc;
use image::apply_image_doc;
use audio::apply_audio_doc;
use tabular::apply_tabular_doc;

/// Typed input for [`ApplyRedactionAction`].
pub struct ApplyRedactionInput {
    /// Text documents to redact.
    pub text_docs: Vec<Document<TxtHandler>>,
    /// Image documents to redact.
    pub image_docs: Vec<Document<PngHandler>>,
    /// Audio documents to redact.
    pub audio_docs: Vec<Document<WavHandler>>,
    /// Tabular documents to redact.
    pub tabular_docs: Vec<Document<CsvHandler>>,
    /// Detected entities referenced by redaction instructions.
    pub entities: Vec<Entity>,
    /// Redaction instructions to apply.
    pub redactions: Vec<Redaction>,
}

/// Typed output for [`ApplyRedactionAction`].
pub struct ApplyRedactionOutput {
    /// Redacted text documents.
    pub text_docs: Vec<Document<TxtHandler>>,
    /// Redacted image documents.
    pub image_docs: Vec<Document<PngHandler>>,
    /// Redacted audio documents.
    pub audio_docs: Vec<Document<WavHandler>>,
    /// Redacted tabular documents.
    pub tabular_docs: Vec<Document<CsvHandler>>,
}

/// Applies pending [`Redaction`] instructions to document content.
///
/// Dispatches per-document based on content type:
/// - **Text documents**: byte-offset replacement
/// - **Image documents**: blur/block overlay
/// - **Audio documents**: stub pass-through
/// - **Tabular documents**: cell-level redaction
pub struct ApplyRedactionAction;

impl ApplyRedactionAction {
    pub async fn connect() -> Result<Self, Error> {
        Ok(Self)
    }

    pub async fn execute(
        &self,
        input: ApplyRedactionInput,
    ) -> Result<ApplyRedactionOutput, Error> {
        let entity_map: HashMap<Uuid, &Entity> =
            input.entities.iter().map(|e| (e.source.as_uuid(), e)).collect();
        let redaction_map: HashMap<Uuid, &Redaction> = input.redactions
            .iter()
            .filter(|r| !r.applied)
            .map(|r| (r.entity_id, r))
            .collect();

        // Text documents
        let mut result_text = Vec::with_capacity(input.text_docs.len());
        for doc in &input.text_docs {
            result_text.push(apply_text_doc(doc, &entity_map, &redaction_map).await?);
        }

        // Image documents
        let mut result_image = Vec::with_capacity(input.image_docs.len());
        for doc in &input.image_docs {
            result_image.push(apply_image_doc(doc, &entity_map, &redaction_map).await?);
        }

        // Audio documents
        let mut result_audio = Vec::with_capacity(input.audio_docs.len());
        for doc in &input.audio_docs {
            result_audio.push(apply_audio_doc(doc, &entity_map, &redaction_map).await?);
        }

        // Tabular documents
        let mut result_tabular = Vec::with_capacity(input.tabular_docs.len());
        for doc in &input.tabular_docs {
            result_tabular.push(apply_tabular_doc(doc, &entity_map, &redaction_map).await?);
        }

        Ok(ApplyRedactionOutput {
            text_docs: result_text,
            image_docs: result_image,
            audio_docs: result_audio,
            tabular_docs: result_tabular,
        })
    }
}

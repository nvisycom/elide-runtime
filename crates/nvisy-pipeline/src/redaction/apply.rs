//! Unified redaction action -- applies text, image, tabular, and audio redactions.

use std::collections::HashMap;
use uuid::Uuid;
use serde::Deserialize;

use nvisy_codec::handler::{TxtHandler, CsvHandler};
use nvisy_codec::document::Document;
use nvisy_codec::render::text::{TextRedaction, AsRedactableText, mask_cell};
use nvisy_codec::render::output::RedactionOutput;
use crate::ontology::redaction::Redaction;
use crate::ontology::entity::Entity;
use nvisy_core::error::Error;

#[cfg(feature = "image-redaction")]
use nvisy_codec::handler::PngHandler;
#[cfg(feature = "image-redaction")]
use nvisy_codec::render::image::{ImageRedaction, AsRedactableImage};

#[cfg(feature = "audio-redaction")]
use nvisy_codec::handler::WavHandler;

use crate::action::Action;

/// Typed parameters for [`ApplyRedactionAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyRedactionParams {
    /// Duration in seconds to crossfade at silence boundaries (audio redaction).
    #[cfg(feature = "audio-redaction")]
    #[serde(default = "default_crossfade_secs")]
    pub crossfade_secs: f64,
}

#[cfg(feature = "audio-redaction")]
fn default_crossfade_secs() -> f64 {
    0.05
}

/// Typed input for [`ApplyRedactionAction`].
pub struct ApplyRedactionInput {
    /// Text documents to redact.
    pub text_docs: Vec<Document<TxtHandler>>,
    /// Image documents to redact (feature-gated).
    #[cfg(feature = "image-redaction")]
    pub image_docs: Vec<Document<PngHandler>>,
    /// Audio documents to redact (feature-gated).
    #[cfg(feature = "audio-redaction")]
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
    /// Redacted image documents (feature-gated).
    #[cfg(feature = "image-redaction")]
    pub image_docs: Vec<Document<PngHandler>>,
    /// Redacted audio documents (feature-gated).
    #[cfg(feature = "audio-redaction")]
    pub audio_docs: Vec<Document<WavHandler>>,
    /// Redacted tabular documents.
    pub tabular_docs: Vec<Document<CsvHandler>>,
}

/// Applies pending [`Redaction`] instructions to document content.
///
/// Dispatches per-document based on content type:
/// - **Text documents**: byte-offset replacement
/// - **Image documents**: blur/block overlay (feature-gated)
/// - **Audio documents**: stub pass-through (feature-gated)
/// - **Tabular documents**: cell-level redaction
pub struct ApplyRedactionAction {
    #[allow(dead_code)]
    params: ApplyRedactionParams,
}

#[async_trait::async_trait]
impl Action for ApplyRedactionAction {
    type Params = ApplyRedactionParams;
    type Input = ApplyRedactionInput;
    type Output = ApplyRedactionOutput;

    fn id(&self) -> &str {
        "apply-redaction"
    }

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        Ok(Self { params })
    }

    async fn execute(
        &self,
        input: Self::Input,
    ) -> Result<Self::Output, Error> {
        let entity_map: HashMap<Uuid, &Entity> =
            input.entities.iter().map(|e| (e.source.as_uuid(), e)).collect();
        let redaction_map: HashMap<Uuid, &Redaction> = input.redactions
            .iter()
            .filter(|r| !r.applied)
            .map(|r| (r.entity_id, r))
            .collect();

        // Text documents
        let mut result_text = Vec::new();
        for doc in &input.text_docs {
            let redacted = apply_text_doc(doc, &entity_map, &redaction_map)?;
            result_text.push(redacted);
        }

        // Image documents
        #[cfg(feature = "image-redaction")]
        let mut result_image = Vec::new();
        #[cfg(feature = "image-redaction")]
        for doc in &input.image_docs {
            let redacted = apply_image_doc(doc, &input.entities, &redaction_map)?;
            result_image.push(redacted);
        }

        // Audio documents
        #[cfg(feature = "audio-redaction")]
        let mut result_audio = Vec::new();
        #[cfg(feature = "audio-redaction")]
        for doc in &input.audio_docs {
            let redacted = apply_audio_doc(doc);
            result_audio.push(redacted);
        }

        // Tabular documents
        let mut result_tabular = Vec::new();
        for doc in &input.tabular_docs {
            let redacted = apply_tabular_doc(doc, &input.entities, &redaction_map);
            result_tabular.push(redacted);
        }

        Ok(ApplyRedactionOutput {
            text_docs: result_text,
            #[cfg(feature = "image-redaction")]
            image_docs: result_image,
            #[cfg(feature = "audio-redaction")]
            audio_docs: result_audio,
            tabular_docs: result_tabular,
        })
    }
}

// ---------------------------------------------------------------------------
// Text redaction
// ---------------------------------------------------------------------------

fn apply_text_doc(
    doc: &Document<TxtHandler>,
    entity_map: &HashMap<Uuid, &Entity>,
    redaction_map: &HashMap<Uuid, &Redaction>,
) -> Result<Document<TxtHandler>, Error> {
    let mut redactions: Vec<TextRedaction> = Vec::new();

    for (entity_id, redaction) in redaction_map {
        let entity = match entity_map.get(entity_id) {
            Some(e) => e,
            None => continue,
        };

        // Check entity belongs to this document
        if entity.source.parent_id() != Some(doc.source.as_uuid()) {
            continue;
        }

        let (start, end) = match &entity.text_location {
            Some(loc) => (loc.start_offset, loc.end_offset),
            None => continue,
        };

        let output = match &redaction.output {
            RedactionOutput::Text(t) => t.clone(),
            _ => continue,
        };

        redactions.push(TextRedaction { start, end, output });
    }

    if redactions.is_empty() {
        return Ok(doc.clone());
    }

    let handler = doc.handler().redact(&redactions)?;
    let mut result = Document::new(handler);
    result.source.set_parent_id(Some(doc.source.as_uuid()));
    Ok(result)
}

// ---------------------------------------------------------------------------
// Image redaction (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "image-redaction")]
fn apply_image_doc(
    doc: &Document<PngHandler>,
    entities: &[Entity],
    redaction_map: &HashMap<Uuid, &Redaction>,
) -> Result<Document<PngHandler>, Error> {
    let mut redactions: Vec<ImageRedaction> = Vec::new();

    for entity in entities {
        if let Some(ref img_loc) = entity.image_location {
            if let Some(redaction) = redaction_map.get(&entity.source.as_uuid()) {
                let output = match &redaction.output {
                    RedactionOutput::Image(img) => img.clone(),
                    _ => continue,
                };
                redactions.push(ImageRedaction {
                    bounding_box: img_loc.bounding_box.clone(),
                    output,
                });
            }
        }
    }

    if redactions.is_empty() {
        return Ok(doc.clone());
    }

    let handler = doc.handler().redact(&redactions)?;
    let mut result = Document::new(handler);
    result.source.set_parent_id(Some(doc.source.as_uuid()));
    Ok(result)
}

// ---------------------------------------------------------------------------
// Audio redaction (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "audio-redaction")]
fn apply_audio_doc(doc: &Document<WavHandler>) -> Document<WavHandler> {
    tracing::warn!("audio redaction not yet implemented");
    doc.clone()
}

// ---------------------------------------------------------------------------
// Tabular redaction
// ---------------------------------------------------------------------------

fn apply_tabular_doc(
    doc: &Document<CsvHandler>,
    entities: &[Entity],
    redaction_map: &HashMap<Uuid, &Redaction>,
) -> Document<CsvHandler> {
    let mut result = doc.clone();

    for entity in entities {
        if let Some(ref tab_loc) = entity.tabular_location {
            let (row_idx, col_idx) = (tab_loc.row_index, tab_loc.column_index);
            if let Some(redaction) = redaction_map.get(&entity.source.as_uuid()) {
                let output = match &redaction.output {
                    RedactionOutput::Text(t) => t,
                    _ => continue,
                };
                if let Some(row) = result.handler_mut().rows_mut().get_mut(row_idx) {
                    if let Some(cell) = row.get_mut(col_idx) {
                        *cell = mask_cell(cell, output);
                    }
                }
            }
        }
    }

    result
}


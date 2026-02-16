//! Unified redaction action -- applies text, image, tabular, and audio redactions.

use std::collections::HashMap;
use uuid::Uuid;
use serde::Deserialize;

use nvisy_codec::handler::{TxtHandler, TxtData, CsvHandler};
use nvisy_codec::document::Document;
use nvisy_codec::render::text::{PendingReplacement, apply_replacements, mask_cell};
use nvisy_ontology::entity::Entity;
use nvisy_ontology::redaction::{Redaction, RedactionOutput};
use nvisy_core::error::Error;

#[cfg(feature = "image-redaction")]
use nvisy_codec::handler::{PngHandler, AsImage};
#[cfg(feature = "image-redaction")]
use nvisy_ontology::entity::BoundingBox;
#[cfg(feature = "image-redaction")]
use nvisy_ontology::redaction::ImageRedactionOutput;

#[cfg(feature = "audio-redaction")]
use nvisy_codec::handler::WavHandler;

use crate::action::Action;

/// Typed parameters for [`ApplyRedactionAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyRedactionParams {
    /// Default mask character for text [`Mask`](nvisy_ontology::redaction::TextRedactionOutput::Mask) redactions.
    #[serde(default = "default_mask_char")]
    pub mask_char: char,
    /// Sigma value for gaussian blur (image redaction).
    #[cfg(feature = "image-redaction")]
    #[serde(default = "default_sigma")]
    pub blur_sigma: f32,
    /// RGBA color for block overlays (image redaction).
    #[cfg(feature = "image-redaction")]
    #[serde(default = "default_block_color")]
    pub block_color: [u8; 4],
    /// Pixel block size for pixelation/mosaic (image redaction).
    #[cfg(feature = "image-redaction")]
    #[serde(default = "default_pixelate_block_size")]
    pub pixelate_block_size: u32,
    /// Duration in seconds to crossfade at silence boundaries (audio redaction).
    #[cfg(feature = "audio-redaction")]
    #[serde(default = "default_crossfade_secs")]
    pub crossfade_secs: f64,
}

fn default_mask_char() -> char {
    '*'
}
#[cfg(feature = "image-redaction")]
fn default_sigma() -> f32 {
    15.0
}
#[cfg(feature = "image-redaction")]
fn default_block_color() -> [u8; 4] {
    [0, 0, 0, 255]
}
#[cfg(feature = "image-redaction")]
fn default_pixelate_block_size() -> u32 {
    10
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
            let redacted = apply_text_doc(doc, &entity_map, &redaction_map, &self.params);
            result_text.push(redacted);
        }

        // Image documents
        #[cfg(feature = "image-redaction")]
        let mut result_image = Vec::new();
        #[cfg(feature = "image-redaction")]
        for doc in &input.image_docs {
            let redacted = apply_image_doc(
                doc,
                &input.entities,
                &redaction_map,
                self.params.blur_sigma,
                self.params.block_color,
            )?;
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
            let redacted = apply_tabular_doc(doc, &input.entities, &redaction_map, &self.params);
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
    params: &ApplyRedactionParams,
) -> Document<TxtHandler> {
    let lines = doc.handler().lines();
    let mut content = lines.join("\n");
    if doc.handler().trailing_newline() {
        content.push('\n');
    }

    let mut pending: Vec<PendingReplacement> = Vec::new();

    for (entity_id, redaction) in redaction_map {
        let entity = match entity_map.get(entity_id) {
            Some(e) => e,
            None => continue,
        };

        // Check entity belongs to this document
        let belongs = entity.source.parent_id() == Some(doc.source.as_uuid());
        if !belongs {
            continue;
        }

        let (start_offset, end_offset) = match &entity.text_location {
            Some(loc) => (loc.start_offset, loc.end_offset),
            None => continue,
        };

        let value = match redaction.output.replacement_value() {
            Some(v) => v.to_string(),
            None => {
                let span_len = end_offset.saturating_sub(start_offset);
                params.mask_char.to_string().repeat(span_len)
            }
        };

        pending.push(PendingReplacement {
            start: start_offset,
            end: end_offset,
            value,
        });
    }

    if pending.is_empty() {
        return doc.clone();
    }

    let redacted_content = apply_replacements(&content, &mut pending);

    let trailing_newline = redacted_content.ends_with('\n');
    let new_lines: Vec<String> = redacted_content.lines().map(String::from).collect();
    let handler = TxtHandler::new(TxtData {
        lines: new_lines,
        trailing_newline,
    });
    let mut result = Document::new(handler);
    result.source.set_parent_id(Some(doc.source.as_uuid()));
    result
}

// ---------------------------------------------------------------------------
// Image redaction (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "image-redaction")]
fn apply_image_doc(
    doc: &Document<PngHandler>,
    entities: &[Entity],
    redaction_map: &HashMap<Uuid, &Redaction>,
    blur_sigma: f32,
    block_color: [u8; 4],
) -> Result<Document<PngHandler>, Error> {
    let mut blur_regions: Vec<BoundingBox> = Vec::new();
    let mut block_regions: Vec<BoundingBox> = Vec::new();

    for entity in entities {
        if let Some(ref img_loc) = entity.image_location {
            let bbox = &img_loc.bounding_box;
            if let Some(redaction) = redaction_map.get(&entity.source.as_uuid()) {
                match &redaction.output {
                    RedactionOutput::Image(ImageRedactionOutput::Blur { .. }) => {
                        blur_regions.push(bbox.clone())
                    }
                    RedactionOutput::Image(ImageRedactionOutput::Block { .. }) => {
                        block_regions.push(bbox.clone())
                    }
                    _ => block_regions.push(bbox.clone()),
                }
            }
        }
    }

    if blur_regions.is_empty() && block_regions.is_empty() {
        return Ok(doc.clone());
    }

    let mut handler = doc.handler().clone();
    if !blur_regions.is_empty() {
        handler = handler.blur(&blur_regions, blur_sigma)?;
    }
    if !block_regions.is_empty() {
        handler = handler.block(&block_regions, block_color)?;
    }

    Ok(Document::new(handler))
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
    params: &ApplyRedactionParams,
) -> Document<CsvHandler> {
    let mut result = doc.clone();

    for entity in entities {
        if let Some(ref tab_loc) = entity.tabular_location {
            let (row_idx, col_idx) = (tab_loc.row_index, tab_loc.column_index);
            if let Some(redaction) = redaction_map.get(&entity.source.as_uuid()) {
                if let Some(row) = result.handler_mut().rows_mut().get_mut(row_idx) {
                    if let Some(cell) = row.get_mut(col_idx) {
                        *cell = mask_cell(cell, &redaction.output, params.mask_char);
                    }
                }
            }
        }
    }

    result
}


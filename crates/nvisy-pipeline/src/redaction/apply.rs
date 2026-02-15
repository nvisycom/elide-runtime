//! Unified redaction action -- applies text, image, tabular, and audio redactions.

use std::collections::HashMap;
use uuid::Uuid;
use serde::Deserialize;

use nvisy_ingest::handler::{TxtHandler, TxtData, CsvHandler};
use nvisy_ingest::document::Document;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::redaction::{Redaction, RedactionOutput, TextRedactionOutput};
use nvisy_core::error::Error;

#[cfg(feature = "image-redaction")]
use bytes::Bytes;
#[cfg(feature = "image-redaction")]
use nvisy_ingest::handler::PngHandler;
#[cfg(feature = "image-redaction")]
use nvisy_ontology::entity::BoundingBox;
#[cfg(feature = "image-redaction")]
use nvisy_ontology::redaction::ImageRedactionOutput;
#[cfg(feature = "image-redaction")]
use nvisy_core::error::ErrorKind;

#[cfg(feature = "audio-redaction")]
use nvisy_ingest::handler::WavHandler;

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

/// A single text replacement that has been resolved but not yet applied.
struct PendingRedaction {
    /// Byte offset where the redaction starts in the original text.
    start_offset: usize,
    /// Byte offset where the redaction ends (exclusive) in the original text.
    end_offset: usize,
    /// The string that will replace the original span.
    replacement_value: String,
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

    let mut pending: Vec<PendingRedaction> = Vec::new();

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

        let replacement_value = match redaction.output.replacement_value() {
            Some(v) => v.to_string(),
            None => {
                let span_len = end_offset.saturating_sub(start_offset);
                params.mask_char.to_string().repeat(span_len)
            }
        };

        pending.push(PendingRedaction {
            start_offset,
            end_offset,
            replacement_value,
        });
    }

    if pending.is_empty() {
        return doc.clone();
    }

    let redacted_content = apply_text_redactions(&content, &mut pending);

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

/// Applies a set of pending redactions to `text`, returning the redacted result.
///
/// Replacements are applied right-to-left (descending start offset) so that
/// earlier byte offsets remain valid after each substitution.
fn apply_text_redactions(text: &str, pending: &mut [PendingRedaction]) -> String {
    // Sort by start offset descending (right-to-left) to preserve positions
    pending.sort_by(|a, b| b.start_offset.cmp(&a.start_offset));

    let mut result = text.to_string();
    for redaction in pending.iter() {
        let start = redaction.start_offset.min(result.len());
        let end = redaction.end_offset.min(result.len());
        if start >= end {
            continue;
        }

        result = format!(
            "{}{}{}",
            &result[..start],
            redaction.replacement_value,
            &result[end..]
        );
    }
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
    use crate::redaction::render::{blur, block};

    let image_bytes = doc.handler().bytes();

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

    let dyn_img = image::load_from_memory(image_bytes).map_err(|e| {
        Error::new(ErrorKind::Runtime, format!("image decode failed: {e}"))
    })?;

    let mut result = dyn_img;
    if !blur_regions.is_empty() {
        result = blur::apply_gaussian_blur(&result, &blur_regions, blur_sigma);
    }
    if !block_regions.is_empty() {
        let color = image::Rgba(block_color);
        result = block::apply_block_overlay(&result, &block_regions, color);
    }

    // Encode back to PNG
    let mut buf = std::io::Cursor::new(Vec::new());
    result
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| {
            Error::new(ErrorKind::Runtime, format!("image encode failed: {e}"))
        })?;

    let new_doc = Document::new(PngHandler::new(Bytes::from(buf.into_inner())));
    Ok(new_doc)
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
                        *cell = apply_cell_redaction(cell, &redaction.output, params.mask_char);
                    }
                }
            }
        }
    }

    result
}

fn apply_cell_redaction(cell: &str, output: &RedactionOutput, default_mask: char) -> String {
    match output {
        RedactionOutput::Text(TextRedactionOutput::Mask { mask_char, .. }) => {
            if cell.len() > 4 {
                format!(
                    "{}{}",
                    mask_char.to_string().repeat(cell.len() - 4),
                    &cell[cell.len() - 4..]
                )
            } else {
                mask_char.to_string().repeat(cell.len())
            }
        }
        RedactionOutput::Text(TextRedactionOutput::Remove) => String::new(),
        RedactionOutput::Text(TextRedactionOutput::Hash { .. }) => {
            format!("[HASH:{:x}]", hash_string(cell))
        }
        _ => output
            .replacement_value()
            .map(|v| v.to_string())
            .unwrap_or_else(|| default_mask.to_string().repeat(cell.len())),
    }
}

fn hash_string(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

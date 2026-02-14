//! Unified redaction action -- applies text, image, tabular, and audio redactions.

use std::collections::HashMap;
use uuid::Uuid;
use serde::Deserialize;

use nvisy_ingest::handler::{FormatHandler, TxtHandler};
use nvisy_ingest::document::Document;
use nvisy_ingest::document::data::*;
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

use crate::action::Action;

/// Typed parameters for [`ApplyRedactionAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyRedactionParams {
    /// Sigma value for gaussian blur (image redaction).
    #[serde(default = "default_sigma")]
    pub blur_sigma: f32,
    /// RGBA color for block overlays (image redaction).
    #[serde(default = "default_color")]
    pub block_color: [u8; 4],
}

fn default_sigma() -> f32 {
    15.0
}
fn default_color() -> [u8; 4] {
    [0, 0, 0, 255]
}

/// Applies pending [`Redaction`] instructions to document content.
///
/// Dispatches per-document based on content type:
/// - **Text documents**: byte-offset replacement
/// - **Image documents**: blur/block overlay (feature-gated)
/// - **Tabular documents**: cell-level redaction
/// - **Audio documents**: pass-through with warning
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
    type Input = (Vec<Document<FormatHandler>>, Vec<Entity>, Vec<Redaction>);
    type Output = Vec<Document<FormatHandler>>;

    fn id(&self) -> &str {
        "apply-redaction"
    }

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        Ok(Self { params })
    }

    async fn execute(
        &self,
        input: Self::Input,
    ) -> Result<Vec<Document<FormatHandler>>, Error> {
        let (documents, entities, redactions) = input;

        let entity_map: HashMap<Uuid, &Entity> =
            entities.iter().map(|e| (e.source.as_uuid(), e)).collect();
        let redaction_map: HashMap<Uuid, &Redaction> = redactions
            .iter()
            .filter(|r| !r.applied)
            .map(|r| (r.entity_id, r))
            .collect();

        let mut result_docs = Vec::new();

        for doc in &documents {
            // Tabular documents
            if doc.tabular().is_some() {
                let redacted = apply_tabular_doc(doc, &entities, &redaction_map);
                result_docs.push(redacted);
                continue;
            }

            // Image documents
            #[cfg(feature = "image-redaction")]
            if doc.image().is_some() {
                let redacted = apply_image_doc(
                    doc,
                    &entities,
                    &redaction_map,
                    self.params.blur_sigma,
                    self.params.block_color,
                )?;
                result_docs.push(redacted);
                continue;
            }

            // Text documents (content present)
            if let Some(content) = doc.text() {
                let redacted = apply_text_doc(
                    doc,
                    content,
                    &entity_map,
                    &redaction_map,
                );
                result_docs.push(redacted);
                continue;
            }

            // Fallback: pass through unchanged
            result_docs.push(doc.clone());
        }

        Ok(result_docs)
    }
}

// ---------------------------------------------------------------------------
// Text redaction
// ---------------------------------------------------------------------------

fn apply_text_doc(
    doc: &Document<FormatHandler>,
    content: &str,
    entity_map: &HashMap<Uuid, &Entity>,
    redaction_map: &HashMap<Uuid, &Redaction>,
) -> Document<FormatHandler> {
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

        let start_offset = match entity.location.start_offset() {
            Some(s) => s,
            None => continue,
        };
        let end_offset = match entity.location.end_offset() {
            Some(e) => e,
            None => continue,
        };

        let replacement_value = redaction
            .output
            .replacement_value()
            .unwrap_or("")
            .to_string();

        pending.push(PendingRedaction {
            start_offset,
            end_offset,
            replacement_value,
        });
    }

    if pending.is_empty() {
        return doc.clone();
    }

    let redacted_content = apply_text_redactions(content, &mut pending);
    let mut result = Document::new(
        FormatHandler::Txt(TxtHandler),
        DocumentData::Text(TextData { text: redacted_content }),
    );
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
    doc: &Document<FormatHandler>,
    entities: &[Entity],
    redaction_map: &HashMap<Uuid, &Redaction>,
    blur_sigma: f32,
    block_color: [u8; 4],
) -> Result<Document<FormatHandler>, Error> {
    use crate::render::{blur, block};

    let image_data = match doc.image() {
        Some(d) => d,
        None => return Ok(doc.clone()),
    };

    let mut blur_regions: Vec<BoundingBox> = Vec::new();
    let mut block_regions: Vec<BoundingBox> = Vec::new();

    for entity in entities {
        if let Some(bbox) = entity.location.bounding_box() {
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

    let dyn_img = image::load_from_memory(&image_data.bytes).map_err(|e| {
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

    let new_doc = Document::new(
        FormatHandler::Png(PngHandler),
        DocumentData::Image(ImageData {
            bytes: Bytes::from(buf.into_inner()),
            mime_type: "image/png".to_string(),
            width: result.width(),
            height: result.height(),
        }),
    );

    Ok(new_doc)
}

// ---------------------------------------------------------------------------
// Tabular redaction
// ---------------------------------------------------------------------------

fn apply_tabular_doc(
    doc: &Document<FormatHandler>,
    entities: &[Entity],
    redaction_map: &HashMap<Uuid, &Redaction>,
) -> Document<FormatHandler> {
    let mut result = doc.clone();

    for entity in entities {
        if let (Some(row_idx), Some(col_idx)) =
            (entity.location.row_index(), entity.location.column_index())
        {
            if let Some(redaction) = redaction_map.get(&entity.source.as_uuid()) {
                if let Some(tabular) = result.tabular_mut() {
                    if let Some(row) = tabular.rows.get_mut(row_idx) {
                        if let Some(cell) = row.get_mut(col_idx) {
                            *cell = apply_cell_redaction(cell, &redaction.output);
                        }
                    }
                }
            }
        }
    }

    result
}

fn apply_cell_redaction(cell: &str, output: &RedactionOutput) -> String {
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
        _ => output.replacement_value().unwrap_or("").to_string(),
    }
}

fn hash_string(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

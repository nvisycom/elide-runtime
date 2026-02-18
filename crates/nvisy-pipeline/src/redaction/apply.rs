//! Unified redaction action -- applies text, image, tabular, and audio redactions.

use std::collections::HashMap;
use uuid::Uuid;
use serde::Deserialize;

use nvisy_codec::handler::{TxtHandler, CsvHandler, PngHandler, WavHandler};
use nvisy_codec::document::Document;
use nvisy_codec::handler::TxtSpan;
use nvisy_codec::transform::{TextRedaction, TextRedactionOutput, TextHandler, RedactionOutput};
use nvisy_codec::transform::{ImageRedaction, ImageHandler};
use crate::ontology::{Entity, Redaction};
use nvisy_core::error::Error;

use crate::action::Action;

/// Typed parameters for [`ApplyRedactionAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyRedactionParams {
    /// Duration in seconds to crossfade at silence boundaries (audio redaction).
    #[serde(default = "default_crossfade_secs")]
    pub crossfade_secs: f64,
}

fn default_crossfade_secs() -> f64 {
    0.05
}

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
            let redacted = apply_text_doc(doc, &entity_map, &redaction_map).await?;
            result_text.push(redacted);
        }

        // Image documents
        let mut result_image = Vec::new();
        for doc in &input.image_docs {
            let redacted = apply_image_doc(doc, &input.entities, &redaction_map).await?;
            result_image.push(redacted);
        }

        // Audio documents
        let mut result_audio = Vec::new();
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
            image_docs: result_image,
            audio_docs: result_audio,
            tabular_docs: result_tabular,
        })
    }
}

// ---------------------------------------------------------------------------
// Text redaction
// ---------------------------------------------------------------------------

async fn apply_text_doc(
    doc: &Document<TxtHandler>,
    entity_map: &HashMap<Uuid, &Entity>,
    redaction_map: &HashMap<Uuid, &Redaction>,
) -> Result<Document<TxtHandler>, Error> {
    // Collect global-offset redactions for this document.
    let mut global_redactions: Vec<(usize, usize, TextRedactionOutput)> = Vec::new();

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

        global_redactions.push((start, end, output));
    }

    if global_redactions.is_empty() {
        return Ok(doc.clone());
    }

    // Build cumulative byte-offset map from lines so we can convert
    // global offsets to (TxtSpan, intra-line start, intra-line end).
    let lines = doc.handler().lines();
    // Each line contributes `line.len()` bytes plus 1 for the '\n' separator.
    let mut line_starts: Vec<usize> = Vec::with_capacity(lines.len());
    let mut offset = 0usize;
    for line in lines {
        line_starts.push(offset);
        // +1 for the '\n' that separates lines in the flat representation
        offset += line.len() + 1;
    }

    // Map each global-offset redaction to per-span redactions, splitting
    // across line boundaries when necessary.
    let mut redactions: Vec<TextRedaction<TxtSpan>> = Vec::new();
    for (g_start, g_end, output) in &global_redactions {
        let g_start = *g_start;
        let g_end = *g_end;

        for (i, &line_start) in line_starts.iter().enumerate() {
            let line_end = line_start + lines[i].len(); // exclusive, before '\n'

            // Skip lines entirely before or after this redaction range.
            if g_end <= line_start || g_start > line_end {
                continue;
            }

            let intra_start = g_start.saturating_sub(line_start);
            let intra_end = if g_end < line_end {
                g_end - line_start
            } else {
                lines[i].len()
            };

            if intra_start >= intra_end {
                continue;
            }

            // Only the first segment of a cross-line redaction carries the
            // replacement value; subsequent segments are removals so that
            // the original text is deleted without duplicating the replacement.
            let seg_output = if line_start <= g_start {
                output.clone()
            } else {
                TextRedactionOutput::Remove
            };

            redactions.push(TextRedaction {
                span_id: TxtSpan(i),
                start: intra_start,
                end: intra_end,
                output: seg_output,
            });
        }
    }

    let mut result = doc.clone();
    result.handler_mut().redact_spans(&redactions).await?;
    result.source.set_parent_id(Some(doc.source.as_uuid()));
    Ok(result)
}

// ---------------------------------------------------------------------------
// Image redaction
// ---------------------------------------------------------------------------

async fn apply_image_doc(
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

    let mut result = doc.clone();
    result.handler_mut().redact_spans(&redactions).await?;
    result.source.set_parent_id(Some(doc.source.as_uuid()));
    Ok(result)
}

// ---------------------------------------------------------------------------
// Audio redaction
// ---------------------------------------------------------------------------

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
                        *cell = output.mask_cell(cell);
                    }
                }
            }
        }
    }

    result
}

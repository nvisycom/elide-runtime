//! Content redaction operation.
//!
//! [`Redaction`] applies pending redaction instructions to document content,
//! dispatching per-document based on content type:
//!
//! | Modality | Handler        | Strategy                            |
//! |----------|----------------|-------------------------------------|
//! | Text     | [`TxtHandler`] | Byte-offset span replacement        |
//! | Image    | [`PngHandler`] | Bounding-box blur/block/pixelate    |
//! | Audio    | [`WavHandler`] | Time-range silence/remove           |
//! | Tabular  | [`CsvHandler`] | Cell-level mask/remove/hash         |

use std::collections::HashMap;
use uuid::Uuid;

use nvisy_codec::document::Document;
use nvisy_codec::handler::{CsvHandler, PngHandler, TxtHandler, TxtSpan, WavHandler};
use nvisy_codec::transform::{
    AudioRedact, AudioRedaction, AudioRedactionOutput, ImageRedact, ImageRedaction,
    ImageRedactionOutput, TextRedact, TextRedaction, TextRedactionOutput,
};
use nvisy_core::Error;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::location::Location;
use nvisy_ontology::record::Redaction as RedactionRecord;
use nvisy_ontology::specification::{
    AudioRedactionInput, ImageRedactionInput, RedactionInput as RedactionSpec, TextRedactionInput,
};

use crate::operation::Operation;

/// Typed input for the [`Redaction`] operation.
pub struct RedactionInput {
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
    pub redactions: Vec<RedactionRecord>,
}

/// Typed output from the [`Redaction`] operation.
pub struct RedactionOutput {
    /// Redacted text documents.
    pub text_docs: Vec<Document<TxtHandler>>,
    /// Redacted image documents.
    pub image_docs: Vec<Document<PngHandler>>,
    /// Redacted audio documents.
    pub audio_docs: Vec<Document<WavHandler>>,
    /// Redacted tabular documents.
    pub tabular_docs: Vec<Document<CsvHandler>>,
}

/// Applies pending redaction instructions to document content.
pub struct Redaction;

impl Operation for Redaction {
    type Input = RedactionInput;
    type Output = RedactionOutput;
    type Context = ();

    async fn call(
        &self,
        input: Self::Input,
        _ctx: Self::Context,
    ) -> Result<Self::Output, Error> {
        let entity_map: HashMap<Uuid, &Entity> =
            input.entities.iter().map(|e| (e.source.as_uuid(), e)).collect();
        let redaction_map: HashMap<Uuid, &RedactionRecord> = input
            .redactions
            .iter()
            .filter(|r| !r.applied)
            .map(|r| (r.entity_id, r))
            .collect();

        let mut result_text = Vec::with_capacity(input.text_docs.len());
        for doc in input.text_docs {
            result_text.push(apply_text_doc(doc, &entity_map, &redaction_map).await?);
        }

        let mut result_image = Vec::with_capacity(input.image_docs.len());
        for doc in input.image_docs {
            result_image.push(apply_image_doc(doc, &entity_map, &redaction_map).await?);
        }

        let mut result_audio = Vec::with_capacity(input.audio_docs.len());
        for doc in input.audio_docs {
            result_audio.push(apply_audio_doc(doc, &entity_map, &redaction_map).await?);
        }

        let mut result_tabular = Vec::with_capacity(input.tabular_docs.len());
        for doc in input.tabular_docs {
            result_tabular.push(apply_tabular_doc(doc, &entity_map, &redaction_map).await?);
        }

        Ok(RedactionOutput {
            text_docs: result_text,
            image_docs: result_image,
            audio_docs: result_audio,
            tabular_docs: result_tabular,
        })
    }
}

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

fn text_output_from_spec(spec: &RedactionSpec, replacement: &str) -> Option<TextRedactionOutput> {
    match spec {
        RedactionSpec::Text(TextRedactionInput::Remove) if replacement.is_empty() => {
            Some(TextRedactionOutput::Remove)
        }
        RedactionSpec::Text(_) => Some(TextRedactionOutput::Replace {
            replacement: replacement.to_string(),
        }),
        _ => None,
    }
}

async fn apply_text_doc(
    mut doc: Document<TxtHandler>,
    entity_map: &HashMap<Uuid, &Entity>,
    redaction_map: &HashMap<Uuid, &RedactionRecord>,
) -> Result<Document<TxtHandler>, Error> {
    let mut global_redactions: Vec<(usize, usize, TextRedactionOutput)> =
        Vec::with_capacity(redaction_map.len());

    for (entity_id, redaction) in redaction_map {
        let entity = match entity_map.get(entity_id) {
            Some(e) => e,
            None => continue,
        };

        if entity.source.parent_id() != Some(doc.source.as_uuid()) {
            continue;
        }

        let (start, end) = match &entity.location {
            Some(Location::Text(loc)) => (loc.start_offset, loc.end_offset),
            _ => continue,
        };

        let output = match text_output_from_spec(&redaction.spec, &redaction.replacement) {
            Some(o) => o,
            None => continue,
        };

        global_redactions.push((start, end, output));
    }

    if global_redactions.is_empty() {
        return Ok(doc);
    }

    let lines = doc.handler().lines();
    let mut line_starts: Vec<usize> = Vec::with_capacity(lines.len());
    let mut offset = 0usize;
    for line in lines {
        line_starts.push(offset);
        offset += line.len() + 1;
    }

    let mut redactions: Vec<TextRedaction<TxtSpan>> = Vec::new();
    for (g_start, g_end, output) in &global_redactions {
        let g_start = *g_start;
        let g_end = *g_end;
        let mut is_first_segment = true;

        for (i, &line_start) in line_starts.iter().enumerate() {
            let line_end = line_start + lines[i].len();

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

            let seg_output = if is_first_segment {
                is_first_segment = false;
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

    doc.handler_mut().redact_text(&redactions).await?;
    Ok(doc)
}

// ---------------------------------------------------------------------------
// Image helpers
// ---------------------------------------------------------------------------

fn image_output_from_spec(spec: &RedactionSpec) -> Option<ImageRedactionOutput> {
    match spec {
        RedactionSpec::Image(img) => Some(match img {
            ImageRedactionInput::Blur { sigma } => ImageRedactionOutput::Blur { sigma: *sigma },
            ImageRedactionInput::Block { color } => ImageRedactionOutput::Block { color: *color },
            ImageRedactionInput::Pixelate { block_size } => {
                ImageRedactionOutput::Pixelate { block_size: *block_size }
            }
            ImageRedactionInput::Synthesize => {
                ImageRedactionOutput::Block { color: [0, 0, 0, 255] }
            }
        }),
        _ => None,
    }
}

async fn apply_image_doc(
    mut doc: Document<PngHandler>,
    entity_map: &HashMap<Uuid, &Entity>,
    redaction_map: &HashMap<Uuid, &RedactionRecord>,
) -> Result<Document<PngHandler>, Error> {
    let mut redactions: Vec<ImageRedaction> = Vec::new();

    for (&entity_id, redaction) in redaction_map {
        let entity = match entity_map.get(&entity_id) {
            Some(e) => e,
            None => continue,
        };

        let img_loc = match &entity.location {
            Some(Location::Image(loc)) => loc,
            _ => continue,
        };

        let output = match image_output_from_spec(&redaction.spec) {
            Some(o) => o,
            None => continue,
        };

        redactions.push(ImageRedaction {
            bounding_box: img_loc.bounding_box,
            output,
        });
    }

    if redactions.is_empty() {
        return Ok(doc);
    }

    doc.handler_mut().redact_images(&redactions).await?;
    Ok(doc)
}

// ---------------------------------------------------------------------------
// Audio helpers
// ---------------------------------------------------------------------------

fn audio_output_from_spec(spec: &RedactionSpec) -> Option<AudioRedactionOutput> {
    match spec {
        RedactionSpec::Audio(audio) => Some(match audio {
            AudioRedactionInput::Silence => AudioRedactionOutput::Silence,
            AudioRedactionInput::Remove => AudioRedactionOutput::Remove,
            AudioRedactionInput::Synthesize => AudioRedactionOutput::Silence,
        }),
        _ => None,
    }
}

async fn apply_audio_doc(
    mut doc: Document<WavHandler>,
    entity_map: &HashMap<Uuid, &Entity>,
    redaction_map: &HashMap<Uuid, &RedactionRecord>,
) -> Result<Document<WavHandler>, Error> {
    let mut redactions: Vec<AudioRedaction> = Vec::new();

    for (&entity_id, redaction) in redaction_map {
        let entity = match entity_map.get(&entity_id) {
            Some(e) => e,
            None => continue,
        };

        let audio_loc = match &entity.location {
            Some(Location::Audio(loc)) => loc,
            _ => continue,
        };

        let output = match audio_output_from_spec(&redaction.spec) {
            Some(o) => o,
            None => continue,
        };

        redactions.push(AudioRedaction {
            start_secs: audio_loc.time_span.start_secs,
            end_secs: audio_loc.time_span.end_secs,
            output,
        });
    }

    if redactions.is_empty() {
        return Ok(doc);
    }

    doc.handler_mut().redact_audio(&redactions).await?;
    Ok(doc)
}

// ---------------------------------------------------------------------------
// Tabular helpers
// ---------------------------------------------------------------------------

async fn apply_tabular_doc(
    mut doc: Document<CsvHandler>,
    entity_map: &HashMap<Uuid, &Entity>,
    redaction_map: &HashMap<Uuid, &RedactionRecord>,
) -> Result<Document<CsvHandler>, Error> {
    for (&entity_id, redaction) in redaction_map {
        let entity = match entity_map.get(&entity_id) {
            Some(e) => e,
            None => continue,
        };

        let tab_loc = match &entity.location {
            Some(Location::Tabular(loc)) => loc,
            _ => continue,
        };

        if !matches!(redaction.spec, RedactionSpec::Text(_)) {
            continue;
        }

        let (row_idx, col_idx) = (tab_loc.row_index, tab_loc.column_index);
        if let Some(row) = doc.handler_mut().rows_mut().get_mut(row_idx)
            && let Some(cell) = row.get_mut(col_idx)
        {
            *cell = mask_cell(&redaction.spec, &redaction.replacement, cell);
        }
    }

    Ok(doc)
}

fn mask_cell(spec: &RedactionSpec, replacement: &str, cell: &str) -> String {
    match spec {
        RedactionSpec::Text(TextRedactionInput::Mask { mask_char }) => {
            let char_count = cell.chars().count();
            if char_count > 4 {
                let masked: String = cell
                    .chars()
                    .take(char_count - 4)
                    .map(|_| *mask_char)
                    .collect();
                let tail: String = cell.chars().skip(char_count - 4).collect();
                format!("{masked}{tail}")
            } else {
                mask_char.to_string().repeat(char_count)
            }
        }
        RedactionSpec::Text(TextRedactionInput::Remove) => String::new(),
        RedactionSpec::Text(TextRedactionInput::Hash) => {
            format!("[HASH:{:x}]", hash_string(cell))
        }
        _ => replacement.to_string(),
    }
}

fn hash_string(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nvisy_ontology::specification::ImageRedactionInput;

    // Text spec tests

    #[test]
    fn text_output_remove_empty_replacement() {
        let spec = RedactionSpec::Text(TextRedactionInput::Remove);
        let output = text_output_from_spec(&spec, "");
        assert_eq!(output, Some(TextRedactionOutput::Remove));
    }

    #[test]
    fn text_output_remove_with_replacement_becomes_replace() {
        let spec = RedactionSpec::Text(TextRedactionInput::Remove);
        let output = text_output_from_spec(&spec, "XXX");
        assert_eq!(
            output,
            Some(TextRedactionOutput::Replace {
                replacement: "XXX".to_string(),
            })
        );
    }

    #[test]
    fn text_output_mask_produces_replace() {
        let spec = RedactionSpec::Text(TextRedactionInput::Mask { mask_char: '*' });
        let output = text_output_from_spec(&spec, "****");
        assert_eq!(
            output,
            Some(TextRedactionOutput::Replace {
                replacement: "****".to_string(),
            })
        );
    }

    #[test]
    fn text_output_image_spec_returns_none() {
        let spec = RedactionSpec::Image(ImageRedactionInput::Blur { sigma: 10.0 });
        assert_eq!(text_output_from_spec(&spec, ""), None);
    }

    // Image spec tests

    #[test]
    fn image_output_blur() {
        let spec = RedactionSpec::Image(ImageRedactionInput::Blur { sigma: 5.0 });
        assert_eq!(
            image_output_from_spec(&spec),
            Some(ImageRedactionOutput::Blur { sigma: 5.0 })
        );
    }

    #[test]
    fn image_output_block() {
        let spec = RedactionSpec::Image(ImageRedactionInput::Block {
            color: [255, 0, 0, 255],
        });
        assert_eq!(
            image_output_from_spec(&spec),
            Some(ImageRedactionOutput::Block {
                color: [255, 0, 0, 255]
            })
        );
    }

    #[test]
    fn image_output_pixelate() {
        let spec = RedactionSpec::Image(ImageRedactionInput::Pixelate { block_size: 8 });
        assert_eq!(
            image_output_from_spec(&spec),
            Some(ImageRedactionOutput::Pixelate { block_size: 8 })
        );
    }

    #[test]
    fn image_output_synthesize_maps_to_black_block() {
        let spec = RedactionSpec::Image(ImageRedactionInput::Synthesize);
        assert_eq!(
            image_output_from_spec(&spec),
            Some(ImageRedactionOutput::Block {
                color: [0, 0, 0, 255]
            })
        );
    }

    #[test]
    fn image_output_text_spec_returns_none() {
        let spec = RedactionSpec::Text(TextRedactionInput::Remove);
        assert_eq!(image_output_from_spec(&spec), None);
    }

    #[test]
    fn image_output_audio_spec_returns_none() {
        let spec =
            RedactionSpec::Audio(nvisy_ontology::specification::AudioRedactionInput::Silence);
        assert_eq!(image_output_from_spec(&spec), None);
    }

    // Audio spec tests

    #[test]
    fn audio_output_silence() {
        let spec = RedactionSpec::Audio(AudioRedactionInput::Silence);
        assert_eq!(audio_output_from_spec(&spec), Some(AudioRedactionOutput::Silence));
    }

    #[test]
    fn audio_output_remove() {
        let spec = RedactionSpec::Audio(AudioRedactionInput::Remove);
        assert_eq!(audio_output_from_spec(&spec), Some(AudioRedactionOutput::Remove));
    }

    #[test]
    fn audio_output_synthesize_falls_back_to_silence() {
        let spec = RedactionSpec::Audio(AudioRedactionInput::Synthesize);
        assert_eq!(audio_output_from_spec(&spec), Some(AudioRedactionOutput::Silence));
    }

    #[test]
    fn audio_output_text_spec_returns_none() {
        let spec = RedactionSpec::Text(TextRedactionInput::Remove);
        assert_eq!(audio_output_from_spec(&spec), None);
    }

    // Tabular spec tests

    #[test]
    fn mask_cell_mask_long() {
        let spec = RedactionSpec::Text(TextRedactionInput::Mask { mask_char: '*' });
        assert_eq!(mask_cell(&spec, "", "1234567890"), "******7890");
    }

    #[test]
    fn mask_cell_mask_short() {
        let spec = RedactionSpec::Text(TextRedactionInput::Mask { mask_char: '#' });
        assert_eq!(mask_cell(&spec, "", "abcd"), "####");
    }

    #[test]
    fn mask_cell_mask_exact_four() {
        let spec = RedactionSpec::Text(TextRedactionInput::Mask { mask_char: 'X' });
        assert_eq!(mask_cell(&spec, "", "1234"), "XXXX");
    }

    #[test]
    fn mask_cell_remove() {
        let spec = RedactionSpec::Text(TextRedactionInput::Remove);
        assert_eq!(mask_cell(&spec, "", "sensitive"), "");
    }

    #[test]
    fn mask_cell_hash() {
        let spec = RedactionSpec::Text(TextRedactionInput::Hash);
        let result = mask_cell(&spec, "", "hello");
        assert!(result.starts_with("[HASH:"));
        assert!(result.ends_with(']'));
    }

    #[test]
    fn mask_cell_replace_fallback() {
        let spec = RedactionSpec::Text(TextRedactionInput::Replace {
            placeholder: String::new(),
        });
        assert_eq!(mask_cell(&spec, "[REDACTED]", "sensitive"), "[REDACTED]");
    }

    #[test]
    fn hash_string_deterministic() {
        assert_eq!(hash_string("hello"), hash_string("hello"));
        assert_ne!(hash_string("hello"), hash_string("world"));
    }
}

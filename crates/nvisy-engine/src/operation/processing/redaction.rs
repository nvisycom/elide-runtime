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

use nvisy_codec::handler::{CsvHandler, Handler, PngHandler, TxtHandler, TxtSpan, WavHandler};
use nvisy_codec::transform::{
    AudioOutput, AudioRedaction, AudioTransform, ImageOutput, ImageRedaction, ImageTransform,
    TextOutput, TextRedaction, TextTransform,
};
use nvisy_core::Result;
use nvisy_ontology::entity::{Entity, Location};
use nvisy_ontology::policy::{AudioStrategy, ImageStrategy, Strategy, TextStrategy};
use uuid::Uuid;

use crate::operation::{Operation, ParallelContext};
use crate::provenance::RedactionDecision;

const TARGET: &str = "nvisy_engine::op::redaction";

/// Typed input for the [`Redaction`] operation.
pub struct RedactionInput {
    /// Text documents to redact.
    pub text_docs: Vec<TxtHandler>,
    /// Image documents to redact.
    pub image_docs: Vec<PngHandler>,
    /// Audio documents to redact.
    pub audio_docs: Vec<WavHandler>,
    /// Tabular documents to redact.
    pub tabular_docs: Vec<CsvHandler>,
    /// Detected entities referenced by redaction instructions.
    pub entities: Vec<Entity>,
    /// Redaction instructions to apply.
    pub decisions: Vec<RedactionDecision>,
}

/// Typed output from the [`Redaction`] operation.
pub struct RedactionOutput {
    /// Redacted text documents.
    pub text_docs: Vec<TxtHandler>,
    /// Redacted image documents.
    pub image_docs: Vec<PngHandler>,
    /// Redacted audio documents.
    pub audio_docs: Vec<WavHandler>,
    /// Redacted tabular documents.
    pub tabular_docs: Vec<CsvHandler>,
}

/// Applies pending redaction instructions to document content.
pub struct Redaction;

impl Redaction {
    async fn execute(&self, input: RedactionInput) -> Result<RedactionOutput> {
        tracing::debug!(
            target: TARGET,
            decisions = input.decisions.len(),
            entities = input.entities.len(),
            "applying redactions",
        );
        let entity_map: HashMap<Uuid, &Entity> = input
            .entities
            .iter()
            .map(|e| (e.source.as_uuid(), e))
            .collect();
        let redaction_map: HashMap<Uuid, &RedactionDecision> = input
            .decisions
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

impl Operation for Redaction {
    type Input = ParallelContext<RedactionInput>;
    type Output = ParallelContext<RedactionOutput>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.execute(data)).await
    }
}

fn text_output_from_spec(spec: &Strategy, replacement: &str) -> Option<TextOutput> {
    match spec {
        Strategy::Text(TextStrategy::Remove) if replacement.is_empty() => Some(TextOutput::Remove),
        Strategy::Text(_) => Some(TextOutput::Replace {
            replacement: replacement.to_string(),
        }),
        _ => None,
    }
}

async fn apply_text_doc(
    mut doc: TxtHandler,
    entity_map: &HashMap<Uuid, &Entity>,
    redaction_map: &HashMap<Uuid, &RedactionDecision>,
) -> Result<TxtHandler> {
    let mut global_redactions: Vec<(usize, usize, TextOutput)> =
        Vec::with_capacity(redaction_map.len());

    for (entity_id, redaction) in redaction_map {
        let entity = match entity_map.get(entity_id) {
            Some(e) => e,
            None => continue,
        };

        if entity.source.parent_id() != Some(doc.source().as_uuid()) {
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

    let lines = doc.lines();
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
                TextOutput::Remove
            };

            redactions.push(TextRedaction {
                span_id: TxtSpan(i),
                start: intra_start,
                end: intra_end,
                output: seg_output,
            });
        }
    }

    doc.redact_text(&redactions).await?;
    Ok(doc)
}

fn image_output_from_spec(spec: &Strategy) -> Option<ImageOutput> {
    match spec {
        Strategy::Image(img) => Some(match img {
            ImageStrategy::Blur { sigma } => ImageOutput::Blur { sigma: *sigma },
            ImageStrategy::Block { color } => ImageOutput::Block { color: *color },
            ImageStrategy::Pixelate { block_size } => ImageOutput::Pixelate {
                block_size: *block_size,
            },
        }),
        _ => None,
    }
}

async fn apply_image_doc(
    mut doc: PngHandler,
    entity_map: &HashMap<Uuid, &Entity>,
    redaction_map: &HashMap<Uuid, &RedactionDecision>,
) -> Result<PngHandler> {
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

    doc.redact_images(&redactions).await?;
    Ok(doc)
}

fn audio_output_from_spec(spec: &Strategy) -> Option<AudioOutput> {
    match spec {
        Strategy::Audio(audio) => Some(match audio {
            AudioStrategy::Silence => AudioOutput::Silence,
            AudioStrategy::Remove => AudioOutput::Remove,
        }),
        _ => None,
    }
}

async fn apply_audio_doc(
    mut doc: WavHandler,
    entity_map: &HashMap<Uuid, &Entity>,
    redaction_map: &HashMap<Uuid, &RedactionDecision>,
) -> Result<WavHandler> {
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

    doc.redact_audio(&redactions).await?;
    Ok(doc)
}

async fn apply_tabular_doc(
    mut doc: CsvHandler,
    entity_map: &HashMap<Uuid, &Entity>,
    redaction_map: &HashMap<Uuid, &RedactionDecision>,
) -> Result<CsvHandler> {
    for (&entity_id, redaction) in redaction_map {
        let entity = match entity_map.get(&entity_id) {
            Some(e) => e,
            None => continue,
        };

        let tab_loc = match &entity.location {
            Some(Location::Tabular(loc)) => loc,
            _ => continue,
        };

        if !matches!(redaction.spec, Strategy::Text(_)) {
            continue;
        }

        let (row_idx, col_idx) = (tab_loc.row_index, tab_loc.column_index);
        if let Some(row) = doc.rows_mut().get_mut(row_idx)
            && let Some(cell) = row.get_mut(col_idx)
        {
            *cell = mask_cell(&redaction.spec, &redaction.replacement, cell);
        }
    }

    Ok(doc)
}

fn mask_cell(spec: &Strategy, replacement: &str, cell: &str) -> String {
    match spec {
        Strategy::Text(TextStrategy::Mask { mask_char }) => {
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
        Strategy::Text(TextStrategy::Remove) => String::new(),
        Strategy::Text(TextStrategy::Hash) => {
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

    #[test]
    fn mask_cell_mask_long() {
        let spec = Strategy::Text(TextStrategy::Mask { mask_char: '*' });
        assert_eq!(mask_cell(&spec, "", "1234567890"), "******7890");
    }

    #[test]
    fn mask_cell_mask_short() {
        let spec = Strategy::Text(TextStrategy::Mask { mask_char: '#' });
        assert_eq!(mask_cell(&spec, "", "abcd"), "####");
    }

    #[test]
    fn mask_cell_mask_exact_four() {
        let spec = Strategy::Text(TextStrategy::Mask { mask_char: 'X' });
        assert_eq!(mask_cell(&spec, "", "1234"), "XXXX");
    }

    #[test]
    fn mask_cell_remove() {
        let spec = Strategy::Text(TextStrategy::Remove);
        assert_eq!(mask_cell(&spec, "", "sensitive"), "");
    }

    #[test]
    fn mask_cell_hash() {
        let spec = Strategy::Text(TextStrategy::Hash);
        let result = mask_cell(&spec, "", "hello");
        assert!(result.starts_with("[HASH:"));
        assert!(result.ends_with(']'));
    }

    #[test]
    fn mask_cell_replace_fallback() {
        let spec = Strategy::Text(TextStrategy::Replace {
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

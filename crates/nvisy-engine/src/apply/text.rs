//! Text document redaction.

use std::collections::HashMap;
use uuid::Uuid;

use nvisy_codec::handler::{TxtHandler, TxtSpan};
use nvisy_codec::document::Document;
use nvisy_codec::transform::{TextRedaction, TextRedactionOutput, TextHandler};
use nvisy_detection::{Entity, Location, Redaction, RedactionSpec, TextRedactionSpec};
use nvisy_core::Error;

/// Convert a `RedactionSpec::Text` + replacement string into a codec
/// [`TextRedactionOutput`].
pub(crate) fn text_output_from_spec(spec: &RedactionSpec, replacement: &str) -> Option<TextRedactionOutput> {
    match spec {
        RedactionSpec::Text(TextRedactionSpec::Remove) if replacement.is_empty() => {
            Some(TextRedactionOutput::Remove)
        }
        RedactionSpec::Text(_) => Some(TextRedactionOutput::Replace {
            replacement: replacement.to_string(),
        }),
        _ => None,
    }
}

pub(crate) async fn apply_text_doc(
    doc: &Document<TxtHandler>,
    entity_map: &HashMap<Uuid, &Entity>,
    redaction_map: &HashMap<Uuid, &Redaction>,
) -> Result<Document<TxtHandler>, Error> {
    // Collect global-offset redactions for this document.
    let mut global_redactions: Vec<(usize, usize, TextRedactionOutput)> =
        Vec::with_capacity(redaction_map.len());

    for (entity_id, redaction) in redaction_map {
        let entity = match entity_map.get(entity_id) {
            Some(e) => e,
            None => continue,
        };

        // Check entity belongs to this document.
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
        return Ok(doc.clone());
    }

    // Build cumulative byte-offset map from lines so we can convert
    // global offsets to (TxtSpan, intra-line start, intra-line end).
    let lines = doc.handler().lines();
    let mut line_starts: Vec<usize> = Vec::with_capacity(lines.len());
    let mut offset = 0usize;
    for line in lines {
        line_starts.push(offset);
        // +1 for the '\n' that separates lines in the flat representation.
        offset += line.len() + 1;
    }

    // Map each global-offset redaction to per-span redactions, splitting
    // across line boundaries when necessary.
    //
    // Only the first segment of a cross-line redaction carries the
    // replacement value; subsequent segments use `Remove` so the
    // original text is deleted without duplicating the replacement.
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

    let mut result = doc.clone();
    result.handler_mut().redact_spans(&redactions).await?;
    result.source.set_parent_id(Some(doc.source.as_uuid()));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nvisy_detection::ImageRedactionSpec;

    #[test]
    fn text_output_remove_empty_replacement() {
        let spec = RedactionSpec::Text(TextRedactionSpec::Remove);
        let output = text_output_from_spec(&spec, "");
        assert_eq!(output, Some(TextRedactionOutput::Remove));
    }

    #[test]
    fn text_output_remove_with_replacement_becomes_replace() {
        let spec = RedactionSpec::Text(TextRedactionSpec::Remove);
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
        let spec = RedactionSpec::Text(TextRedactionSpec::Mask { mask_char: '*' });
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
        let spec = RedactionSpec::Image(ImageRedactionSpec::Blur { sigma: 10.0 });
        assert_eq!(text_output_from_spec(&spec, ""), None);
    }
}

//! Incoming wire types for `POST /gliner-2`.
//!
//! Pioneer groups entities by label — `{"entities": {"person": [{...}]}}`
//! — where elide wants a flat list of spans, so the label comes from the
//! map key rather than from each entry.
//!
//! The offsets need converting, not just copying. Pioneer reports
//! **character** positions (its own client documents them as
//! "character-level start/end positions"); [`NerSpan::offset`] is a byte
//! range into the source `&str`. The two coincide only for ASCII, so a
//! multilingual PII model — the reason to use this backend at all — would
//! otherwise silently shift every span in a document containing an
//! accented name.
//!
//! [`NerSpan::offset`]: elide_ner::backend::NerSpan

use std::collections::HashMap;

use elide_ner::backend::{NerResponse, NerSpan};
use serde::Deserialize;

/// The `/gliner-2` response body, in its `format_results` shape.
#[derive(Debug, Deserialize)]
pub(super) struct WireResponse {
    /// Entities grouped by label; absent when the model found none.
    #[serde(default)]
    pub entities: HashMap<String, Vec<WireEntity>>,
}

/// One extracted entity.
#[derive(Debug, Deserialize)]
pub(super) struct WireEntity {
    /// The matched substring. Used to validate the offsets rather than to
    /// carry text: elide slices the source itself.
    pub text: String,
    /// Confidence in `[0, 1]`. Absent unless `include_confidence` was set.
    #[serde(default)]
    pub confidence: Option<f32>,
    /// Character offset, inclusive. Absent unless `include_spans` was set.
    #[serde(default)]
    pub start: Option<usize>,
    /// Character offset, exclusive.
    #[serde(default)]
    pub end: Option<usize>,
}

/// A lookup from character index to byte index for `text`.
///
/// Built once per call and indexed by character position, with a final
/// entry for the end of the string so an exclusive end offset resolves.
fn char_to_byte(text: &str) -> Vec<usize> {
    let mut map: Vec<usize> = text.char_indices().map(|(byte, _)| byte).collect();
    map.push(text.len());
    map
}

impl WireResponse {
    /// Translate into the [`NerResponse`] the backend trait expects.
    ///
    /// `source` is the text that was submitted; the character offsets index
    /// into it, and spans that do not resolve against it are dropped rather
    /// than guessed at.
    pub(super) fn decode(self, source: &str) -> NerResponse {
        let offsets = char_to_byte(source);
        let mut spans = Vec::new();
        for (label, entities) in self.entities {
            spans.extend(
                entities
                    .into_iter()
                    .filter_map(|entity| entity.decode(&label, &offsets)),
            );
        }
        NerResponse::new(spans)
    }
}

impl WireEntity {
    /// One span, or `None` when it cannot be placed in the source.
    ///
    /// A span is dropped rather than repaired when it has no offsets (the
    /// request always asks for them, so their absence means the provider
    /// did not honour it), when it is inverted, or when it runs past the
    /// end of the submitted text. Each of those means the provider and the
    /// caller disagree about the text, and a mis-placed span in a redaction
    /// pipeline redacts the wrong bytes.
    fn decode(self, label: &str, offsets: &[usize]) -> Option<NerSpan> {
        let (start, end) = (self.start?, self.end?);
        // `offsets` has one entry per character plus a final end marker, so
        // a valid exclusive end is at most its last index.
        if end <= start || end >= offsets.len() {
            return None;
        }
        let range = offsets[start]..offsets[end];
        // The provider echoes the matched text. If the span it points at is
        // not that many bytes long, the offsets do not describe the text we
        // sent, and are not ours to trust.
        if range.len() != self.text.len() {
            return None;
        }
        Some(NerSpan::new(
            label.to_owned(),
            self.confidence.unwrap_or(0.0),
            range,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(text: &str, start: usize, end: usize) -> WireEntity {
        WireEntity {
            text: text.to_owned(),
            confidence: Some(0.9),
            start: Some(start),
            end: Some(end),
        }
    }

    fn decode_one(source: &str, label: &str, entity: WireEntity) -> Option<NerSpan> {
        let mut entities = HashMap::new();
        entities.insert(label.to_owned(), vec![entity]);
        WireResponse { entities }
            .decode(source)
            .spans
            .into_iter()
            .next()
    }

    /// Character offsets become byte offsets.
    ///
    /// The whole point of the conversion: "Zoë" is 3 characters but 4
    /// bytes, so a span after it would land a byte early if the character
    /// offsets were used verbatim — redacting the wrong bytes.
    #[test]
    fn character_offsets_become_byte_offsets() {
        let source = "Zoë lives in Paris";
        // "Paris" is characters 13..18, but bytes 14..19.
        let span = decode_one(source, "city", entity("Paris", 13, 18)).unwrap();

        assert_eq!(span.offset, 14..19);
        assert_eq!(&source[span.offset.clone()], "Paris");
    }

    /// Pure ASCII: the two indexings coincide, so nothing shifts.
    #[test]
    fn ascii_offsets_are_unchanged() {
        let source = "Ada lives in Paris";
        let span = decode_one(source, "city", entity("Paris", 13, 18)).unwrap();

        assert_eq!(span.offset, 13..18);
        assert_eq!(&source[span.offset.clone()], "Paris");
    }

    /// The label comes from the map key, since Pioneer groups by label
    /// rather than repeating it on each entity.
    #[test]
    fn label_comes_from_the_group_key() {
        let span = decode_one("Ada lives in Paris", "city", entity("Paris", 13, 18)).unwrap();
        assert_eq!(span.label.as_str(), "city");
    }

    /// A span running past the end of the submitted text is dropped, not
    /// clamped: the provider and the caller disagree about the text, and a
    /// guessed span redacts the wrong bytes.
    #[test]
    fn drops_spans_past_the_end_of_the_text() {
        assert!(decode_one("short", "city", entity("Paris", 13, 18)).is_none());
    }

    /// An inverted or empty span is dropped.
    #[test]
    fn drops_inverted_and_empty_spans() {
        let source = "Ada lives in Paris";
        assert!(decode_one(source, "city", entity("Paris", 18, 13)).is_none());
        assert!(decode_one(source, "city", entity("", 13, 13)).is_none());
    }

    /// Offsets are requested on every call, so their absence means the
    /// provider did not honour the request; the span is dropped rather
    /// than placed at a guess.
    #[test]
    fn drops_entities_without_offsets() {
        let without = WireEntity {
            text: "Paris".to_owned(),
            confidence: Some(0.9),
            start: None,
            end: None,
        };
        assert!(decode_one("Ada lives in Paris", "city", without).is_none());
    }

    /// The echoed text must match the span's byte length, or the offsets
    /// do not describe the text that was sent.
    #[test]
    fn drops_spans_disagreeing_with_the_echoed_text() {
        // Offsets cover "Paris" (5 bytes) but the echo claims "Par".
        assert!(decode_one("Ada lives in Paris", "city", entity("Par", 13, 18)).is_none());
    }
}

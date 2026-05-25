//! [`Span`] — a range of recognized text tagged with its source location.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::Location;

/// A range of text within a [`Chunk`]'s flat `text`, paired with
/// the [`Location`] of where that text came from in the source
/// document.
///
/// Source-mapped granularity is one span per recognized unit:
/// - one span per OCR word for image pages,
/// - one span per cell for tabular rows,
/// - one span per transcribed segment for audio,
/// - one span per text run for natively-extracted text.
///
/// Detection consumes the flat text and produces entity offsets
/// into it; redaction looks up the span at those offsets and
/// dispatches on the [`Location`] variant.
///
/// [`Chunk`]: super::Chunk
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    /// Byte offset into the chunk's `text` where this span starts.
    pub text_start: usize,
    /// Byte offset into the chunk's `text` where this span ends
    /// (exclusive).
    pub text_end: usize,
    /// Confidence in the recognized text (`0.0..=1.0`). Populated
    /// for OCR and STT spans; absent for native text-layer
    /// extractions where the source already provides the text
    /// directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// Where in the source this text came from. Reuses the
    /// canonical [`Location`] enum used by entities so redaction
    /// dispatches uniformly.
    pub source: Location,
}

impl Span {
    /// Byte length of the span in the chunk's `text` (`text_end -
    /// text_start`).
    pub fn len(&self) -> usize {
        self.text_end.saturating_sub(self.text_start)
    }

    /// Returns `true` when the span covers zero bytes.
    pub fn is_empty(&self) -> bool {
        self.text_end <= self.text_start
    }

    /// Returns `true` when `offset` falls within
    /// `text_start..text_end`.
    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.text_start && offset < self.text_end
    }

    /// Returns `true` when this span's text range overlaps
    /// `[start, end)`.
    pub fn overlaps(&self, start: usize, end: usize) -> bool {
        self.text_start < end && self.text_end > start
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::TextLocation;

    fn span(start: usize, end: usize) -> Span {
        Span {
            text_start: start,
            text_end: end,
            confidence: None,
            source: Location::Text(
                TextLocation::builder()
                    .with_start_offset(start)
                    .with_end_offset(end)
                    .build()
                    .unwrap(),
            ),
        }
    }

    #[test]
    fn len_and_emptiness() {
        let s = span(10, 20);
        assert_eq!(s.len(), 10);
        assert!(!s.is_empty());

        let empty = span(10, 10);
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
    }

    #[test]
    fn contains_is_half_open() {
        let s = span(10, 20);
        assert!(!s.contains(9));
        assert!(s.contains(10));
        assert!(s.contains(19));
        assert!(!s.contains(20));
    }

    #[test]
    fn overlap_detects_partial_and_full_overlaps() {
        let s = span(10, 20);
        assert!(s.overlaps(5, 12));    // overlaps start
        assert!(s.overlaps(15, 25));   // overlaps end
        assert!(s.overlaps(12, 18));   // fully inside
        assert!(s.overlaps(0, 100));   // fully contains
        assert!(!s.overlaps(0, 10));   // touches but doesn't overlap
        assert!(!s.overlaps(20, 30));  // touches but doesn't overlap
        assert!(!s.overlaps(0, 5));    // disjoint left
        assert!(!s.overlaps(25, 30));  // disjoint right
    }
}

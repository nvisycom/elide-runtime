//! [`Text`] modality marker, [`TextLocation`] coordinate type,
//! [`TextData`] per-call payload, and [`TextExtraction`] provenance
//! enum.

use std::cmp::Ordering;
use std::ops::Range;

use derive_more::{AsRef, Deref, Display, From};
use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Modality, Overlap};
use crate::entity::ModelProvenance;
use crate::redaction::TextReplacement;

/// Text modality marker (zero-sized).
///
/// Used as the type parameter on generic carriers (`Entity<Text>`,
/// `Hint<Text>`, `RecognizerInput<Text>`, …). The per-call payload
/// (a [`TextLocation`]) is stored as `M::Location` on those carriers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Text;

impl Modality for Text {
    type Data = TextData;
    type Extraction = TextExtraction;
    type Location = TextLocation;
    type Replacement = TextReplacement;

    const KIND: super::ModalityKind = super::ModalityKind::Text;
    const NAME: &'static str = "text";
}

/// Half-open `[start, end)` byte range around a [`TextLocation`],
/// used for the optional surrounding context window. The newtype
/// makes the "both endpoints or none" invariant unrepresentable —
/// the previous twin-`Option` fields allowed a `(Some, None)`
/// half-state with no meaningful semantics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextWindow {
    /// Byte offset where the context window starts.
    pub start: usize,
    /// Byte offset where the context window ends (exclusive).
    pub end: usize,
}

impl ContextWindow {
    /// Construct a window covering `start..end`.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// A range within text content.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextLocation {
    /// Byte or character offset where the range starts.
    pub start: usize,
    /// Byte or character offset where the range ends.
    pub end: usize,
    /// Surrounding context window for redaction, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<ContextWindow>,
    /// 1-based page number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
}

impl TextLocation {
    /// Create a [`TextLocation`] covering `start..end` with all
    /// optional fields unset.
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start,
            end,
            context: None,
            page_number: None,
        }
    }

    /// Byte length of the range (`end - start`).
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Whether the range is empty (zero length).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Translate a value-local byte range to a parent-local
    /// [`TextLocation`], assuming the value is a verbatim slice of
    /// the source covered by `self` (no escapes, no decoding).
    ///
    /// Returns `None` when `value_range.end > self.len()` or the
    /// range is malformed (`start > end`).
    ///
    /// Used by [`Handler::lift_chunk`] implementations whose chunk
    /// data and source bytes coincide — TXT lines, HTML text
    /// nodes, PDF page text, DOCX text runs, etc.
    ///
    /// [`Handler::lift_chunk`]: # "see nvisy-codec"
    #[must_use]
    pub fn subslice(&self, value_range: Range<usize>) -> Option<TextLocation> {
        if value_range.start > value_range.end || value_range.end > self.len() {
            return None;
        }
        Some(TextLocation {
            start: self.start + value_range.start,
            end: self.start + value_range.end,
            context: self.context,
            page_number: self.page_number,
        })
    }
}

impl Ord for TextLocation {
    /// Lex order over `(start, end)`. `context` and `page_number`
    /// are ignored.
    fn cmp(&self, other: &Self) -> Ordering {
        (self.start, self.end).cmp(&(other.start, other.end))
    }
}

impl PartialOrd for TextLocation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// How a [`Document<Text>`]'s text content was produced.
///
/// [`Document<Text>`]: # "carrier owned by nvisy-engine"
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TextExtraction {
    /// Structural parse of a text-bearing format: PDF text layer,
    /// DOCX XML runs, HTML, plain UTF-8.
    Native,
    /// Text obtained by OCR'ing an image-backed page (image-only PDF,
    /// scanned document).
    Recognized(ModelProvenance),
}

/// Per-call payload for [`Text`] recognizers, extractors, and codec
/// chunk reads.
///
/// Held as a [`HipStr<'static>`] so cheap clones (atomic refcount
/// for non-inline text, inline copy for short strings) let the
/// caller share one payload across multiple recognizers without
/// duplicating the source bytes.
///
/// Shared per-call enrichment (lemmatized tokens, language
/// detections, stopword sets) lives on the surrounding
/// [`Span<Text>`]'s [`Artifacts`] bundle, not on [`TextData`] —
/// the same typed-map is reused for every modality and every
/// recognizer/extractor stage.
///
/// [`Span<Text>`]: crate::extraction::Span
/// [`Artifacts`]: crate::extraction::Artifacts
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Display, From, AsRef, Deref)]
#[as_ref(forward)]
#[display("{text}")]
pub struct TextData {
    /// The text the recognizer should scan. Byte offsets in emitted
    /// entities refer back into this string.
    #[deref]
    pub text: HipStr<'static>,
}

impl TextData {
    /// Construct from anything convertible to [`HipStr<'static>`] —
    /// owned `String`, borrowed `&'static str`, an existing
    /// `HipStr`, …
    pub fn new(text: impl Into<HipStr<'static>>) -> Self {
        Self { text: text.into() }
    }

    /// View the inner string slice.
    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }

    /// Consume the wrapper and return the content as a [`String`],
    /// allocating only when the underlying [`HipStr`] is borrowed.
    pub fn into_string(self) -> String {
        self.text.into()
    }
}

impl From<&str> for TextData {
    fn from(s: &str) -> Self {
        Self::new(HipStr::from(s))
    }
}

impl From<String> for TextData {
    fn from(s: String) -> Self {
        Self::new(HipStr::from(s))
    }
}

impl PartialEq<&str> for TextData {
    fn eq(&self, other: &&str) -> bool {
        self.text.as_str() == *other
    }
}

impl Overlap for TextLocation {
    /// Two text ranges overlap only when they share a page (or both
    /// have `page_number: None`) and their byte ranges intersect.
    /// Without the page gate, two ranges on different pages of the
    /// same document that happen to share byte offsets would
    /// false-positive as overlapping.
    fn overlaps(&self, other: &Self) -> bool {
        self.page_number == other.page_number && self.start < other.end && other.start < self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn len_and_is_empty() {
        assert_eq!(TextLocation::new(0, 10).len(), 10);
        assert!(!TextLocation::new(0, 10).is_empty());
        assert!(TextLocation::new(5, 5).is_empty());
    }

    #[test]
    fn overlap_intersecting() {
        assert!(TextLocation::new(0, 10).overlaps(&TextLocation::new(5, 15)));
    }

    #[test]
    fn overlap_contained() {
        assert!(TextLocation::new(0, 10).overlaps(&TextLocation::new(2, 5)));
    }

    #[test]
    fn no_overlap_adjacent() {
        assert!(!TextLocation::new(0, 5).overlaps(&TextLocation::new(5, 10)));
    }

    #[test]
    fn no_overlap_disjoint() {
        assert!(!TextLocation::new(0, 5).overlaps(&TextLocation::new(10, 15)));
    }
}

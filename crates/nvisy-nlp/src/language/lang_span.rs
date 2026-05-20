//! [`LanguageSpan`] — contiguous single-language section within a
//! mixed-language text.

use nvisy_ontology::primitive::LanguageTag;

/// A contiguous single-language section within a possibly
/// mixed-language text. Returned by [`detect_multiple`].
///
/// `start` and `end` are byte offsets into the original text. The
/// language and optional confidence follow the same conventions as
/// [`LanguageDetection`].
///
/// [`detect_multiple`]: super::LanguageDetector::detect_multiple
/// [`LanguageDetection`]: super::LanguageDetection
#[derive(Debug, Clone)]
pub struct LanguageSpan {
    /// Byte offset of the span start in the original text.
    pub start: usize,
    /// Byte offset of the span end in the original text.
    pub end: usize,
    /// The detected language for the span.
    pub language: LanguageTag,
    /// Optional confidence score in `[0.0, 1.0]`.
    pub confidence: Option<f64>,
}

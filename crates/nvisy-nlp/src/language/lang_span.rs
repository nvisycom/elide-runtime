//! [`LanguageSpan`] — byte-offset range a [`LanguageDetection`]
//! applies to.

/// A byte-offset range within the analyzed text.
///
/// Attached to a [`LanguageDetection`] when the detector knows the
/// span its answer covers (mixed-language input produces multiple
/// detections, each with a distinct span). Single-language detections
/// from non-segmenting backends typically leave the span as `None`.
///
/// [`LanguageDetection`]: super::LanguageDetection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageSpan {
    /// Byte offset of the span start in the original text.
    pub start: usize,
    /// Byte offset of the span end in the original text.
    pub end: usize,
}

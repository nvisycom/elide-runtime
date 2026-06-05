//! [`LanguageDetections`]: typed wrapper around a list of
//! [`LanguageDetection`] suitable for storage in a typed-map
//! artifact bundle.
//!
//! Producers (language-detection backends in `nvisy-ner` and other
//! NLP crates) construct one of these for the text they scan and
//! insert it on an [`Artifacts`] map; consumers that care about
//! language (recognizers gating on a language hint, per-language
//! stopword resolution) fetch by type.
//!
//! Whole-document detections store one entry with `span = None`;
//! multi-language documents store one entry per language with the
//! byte range covered.
//!
//! [`Artifacts`]: crate::extraction::Artifacts

use super::LanguageDetection;

/// Languages a language-detection backend resolved for one text scan.
///
/// Newtype around `Vec<LanguageDetection>` so the type-map sees a
/// distinct typed entry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LanguageDetections(pub Vec<LanguageDetection>);

impl LanguageDetections {
    /// Construct from a list of detections.
    #[must_use]
    pub fn new(detections: Vec<LanguageDetection>) -> Self {
        Self(detections)
    }

    /// Borrow the underlying detections.
    #[must_use]
    pub fn as_slice(&self) -> &[LanguageDetection] {
        &self.0
    }

    /// The language covering the most bytes of the source text,
    /// breaking ties on detector confidence.
    ///
    /// Monolingual docs return the single detection; mixed-language
    /// docs return the largest-coverage span; caller-asserted
    /// languages (no `span`) are treated as covering the whole
    /// document and therefore win against any one region.
    ///
    /// Returns `None` iff the list is empty.
    pub fn dominant(&self) -> Option<&LanguageDetection> {
        self.0.iter().max_by(|a, b| {
            span_bytes(a)
                .cmp(&span_bytes(b))
                .then_with(|| confidence_key(a).total_cmp(&confidence_key(b)))
        })
    }
}

impl From<Vec<LanguageDetection>> for LanguageDetections {
    fn from(detections: Vec<LanguageDetection>) -> Self {
        Self::new(detections)
    }
}

fn span_bytes(d: &LanguageDetection) -> usize {
    match d.span {
        Some(s) => s.end.saturating_sub(s.start),
        None => usize::MAX,
    }
}

fn confidence_key(d: &LanguageDetection) -> f64 {
    d.confidence.map(|c| c.get()).unwrap_or(f64::NEG_INFINITY)
}

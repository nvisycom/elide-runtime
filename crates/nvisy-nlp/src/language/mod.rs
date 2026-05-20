//! [`LanguageDetector`] trait, supporting types, and built-in
//! implementations.
//!
//! Detection runs synchronously — language detection on a single
//! string is CPU-bound and fast. The trait is **fallible** so future
//! networked detectors can report real backend failures; pure-CPU
//! detectors that never fail simply return `Ok(...)`.
//!
//! [`detect`](LanguageDetector::detect) returns
//! `Result<Option<LanguageDetection>>` to split two different
//! concepts: `Ok(None)` means *no answer* (text too short or
//! ambiguous — not an error), and `Err(_)` means *real failure*
//! (network, model, etc.). [`detect_multiple`](LanguageDetector::detect_multiple)
//! returns `Result<Vec<LanguageSpan>>` where an empty vec already
//! represents "couldn't decide on any span."

mod lingua;

pub use self::lingua::LinguaLanguageDetector;

use nvisy_ontology::primitive::LanguageTag;

use crate::error::Result;

/// A single language detection result.
///
/// Carries the detected language plus an optional confidence score
/// in the range `[0.0, 1.0]`. Backends that don't expose confidence
/// (or where confidence isn't meaningful) leave it as `None`.
#[derive(Debug, Clone)]
pub struct LanguageDetection {
    /// The detected language.
    pub language: LanguageTag,
    /// Optional confidence score in `[0.0, 1.0]`. `None` when the
    /// backend doesn't expose one.
    pub confidence: Option<f64>,
}

/// A contiguous single-language section within a possibly
/// mixed-language text. Returned by
/// [`detect_multiple`](LanguageDetector::detect_multiple).
///
/// `start` and `end` are byte offsets into the original text. The
/// language and optional confidence follow the same conventions as
/// [`LanguageDetection`].
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

/// Detect the dominant language of a text string.
///
/// `Ok(None)` represents "no answer" (text too short, ambiguous);
/// `Err(_)` represents a real backend failure. Implementations prefer
/// `Ok(None)` over guessing when input is inconclusive.
pub trait LanguageDetector: Send + Sync {
    /// Detect the dominant language of `text`.
    fn detect(&self, text: &str) -> Result<Option<LanguageDetection>>;

    /// Detect contiguous single-language sections within `text`.
    ///
    /// The default implementation falls back to single-language
    /// [`detect`](Self::detect) over the entire string. Backends with
    /// real mixed-language support — [`LinguaLanguageDetector`], for
    /// example — override this with proper segmentation.
    fn detect_multiple(&self, text: &str) -> Result<Vec<LanguageSpan>> {
        Ok(match self.detect(text)? {
            Some(d) => vec![LanguageSpan {
                start: 0,
                end: text.len(),
                language: d.language,
                confidence: d.confidence,
            }],
            None => Vec::new(),
        })
    }
}

//! [`LanguageDetector`] trait, supporting types, and built-in
//! implementations.
//!
//! Detection runs synchronously — language detection on a single
//! string is CPU-bound and fast. The trait is **fallible** so future
//! networked detectors can report real backend failures; pure-CPU
//! detectors that never fail simply return `Ok(...)`.
//!
//! [`detect`] returns `Result<Option<LanguageDetection>>` to split
//! two different concepts: `Ok(None)` means *no answer* (text too
//! short or ambiguous — not an error), and `Err(_)` means *real
//! failure* (network, model, etc.). [`detect_multiple`] returns
//! `Result<Vec<LanguageSpan>>` where an empty vec already represents
//! "couldn't decide on any span."
//!
//! [`detect`]: LanguageDetector::detect
//! [`detect_multiple`]: LanguageDetector::detect_multiple

mod lang_detection;
mod lang_span;
mod lingua;

pub use self::lang_detection::{LanguageDetection, LanguageProvenance};
pub use self::lang_span::LanguageSpan;
pub use self::lingua::LinguaLanguageDetector;

use crate::error::Result;

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
    /// [`detect`] over the entire string. Backends with real
    /// mixed-language support — [`LinguaLanguageDetector`], for
    /// example — override this with proper segmentation.
    ///
    /// [`detect`]: Self::detect
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

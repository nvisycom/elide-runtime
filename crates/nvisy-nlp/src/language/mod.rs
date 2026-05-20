//! [`LanguageDetector`] trait, supporting types, and built-in
//! implementations.
//!
//! Detection runs synchronously — language detection on a single
//! string is CPU-bound and fast. The trait is **fallible** so future
//! networked detectors can report real backend failures; pure-CPU
//! detectors that never fail simply return `Ok(...)`.
//!
//! Both trait methods return `Result<Vec<LanguageDetection>>` so
//! callers see a uniform shape: an empty vec is "couldn't decide,"
//! a single-element vec is the common monolingual answer, and a
//! multi-element vec carries per-region [`LanguageSpan`]s for
//! mixed-language input. The caller picks the first entry when they
//! only need the dominant language.
//!
//! [`detect_in`] is a per-call refinement of [`detect`]: the caller
//! restricts which languages the detector should consider for this
//! one call. The default implementation ignores the restriction and
//! delegates to [`detect`]; backends that natively support a
//! constructable candidate set override it.
//!
//! [`detect`]: LanguageDetector::detect
//! [`detect_in`]: LanguageDetector::detect_in

mod lang_detection;
mod lang_span;
mod lingua;

pub use self::lang_detection::{LanguageDetection, LanguageProvenance};
pub use self::lang_span::LanguageSpan;
pub use self::lingua::LinguaLanguageDetector;

use nvisy_ontology::primitive::LanguageTag;

use crate::error::Result;

/// Detect languages within a text string.
///
/// Implementations return `Ok(vec![])` for inconclusive input (text
/// too short, no recognised script, etc.) and reserve `Err(_)` for
/// real backend failures (network, model load, etc.).
pub trait LanguageDetector: Send + Sync {
    /// Detect languages in `text`.
    ///
    /// Single-language detectors return a one-element vec; backends
    /// with mixed-language support return one entry per detected
    /// region with [`LanguageDetection::span`] populated. An empty
    /// vec means "couldn't decide."
    fn detect(&self, text: &str) -> Result<Vec<LanguageDetection>>;

    /// Detect languages in `text`, restricting the candidate set to
    /// `candidates` for this call only.
    ///
    /// The default implementation ignores `candidates` and delegates
    /// to [`detect`]. Backends that can cheaply construct a per-call
    /// restricted detector — [`LinguaLanguageDetector`], for example
    /// — override this to honor the restriction.
    ///
    /// An empty `candidates` slice is treated as "no restriction"
    /// and behaves identically to [`detect`].
    ///
    /// [`detect`]: Self::detect
    fn detect_in(&self, text: &str, _candidates: &[LanguageTag]) -> Result<Vec<LanguageDetection>> {
        self.detect(text)
    }
}

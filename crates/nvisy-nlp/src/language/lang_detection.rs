//! [`LanguageDetection`] — single-language detection result, with
//! [`LanguageProvenance`] distinguishing detected from caller-asserted.

use nvisy_ontology::primitive::LanguageTag;

/// Provenance of a [`LanguageDetection`].
///
/// Lets consumers distinguish "the engine ran a detector and got
/// this answer" from "the caller asserted this language and bypassed
/// detection" — without overloading `confidence: None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageProvenance {
    /// The language was produced by a [`LanguageDetector`].
    ///
    /// [`LanguageDetector`]: super::LanguageDetector
    Detected,
    /// The language was asserted by the caller (e.g. via
    /// [`Engine::analyze_in_language`]).
    ///
    /// [`Engine::analyze_in_language`]: crate::engine::Engine::analyze_in_language
    Asserted,
}

/// A single language detection result.
///
/// Carries the detected language plus an optional confidence score
/// in the range `[0.0, 1.0]`. Backends that don't expose confidence
/// (or where confidence isn't meaningful) leave it as `None`.
///
/// The `provenance` field records whether this answer came from a
/// real detector run or was asserted by the caller; backends only
/// ever produce [`LanguageProvenance::Detected`], with `Asserted`
/// reserved for the engine when bypassing detection.
#[derive(Debug, Clone)]
pub struct LanguageDetection {
    /// The detected language.
    pub language: LanguageTag,
    /// Optional confidence score in `[0.0, 1.0]`. `None` when the
    /// backend doesn't expose one.
    pub confidence: Option<f64>,
    /// How this language was obtained: detected or caller-asserted.
    pub provenance: LanguageProvenance,
}

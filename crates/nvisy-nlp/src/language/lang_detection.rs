//! [`LanguageDetection`] — single-language detection result, with
//! [`LanguageProvenance`] distinguishing detected from caller-asserted
//! and an optional [`LanguageSpan`] for mixed-language detectors.

use nvisy_ontology::primitive::{Confidence, LanguageTag};

use super::LanguageSpan;

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
    /// [`ContextBuilder::with_language`]).
    ///
    /// [`ContextBuilder::with_language`]: crate::engine::ContextBuilder::with_language
    Asserted,
}

/// A single language detection result.
///
/// Carries the detected language plus an optional confidence and an
/// optional byte-offset [`LanguageSpan`]. Backends that don't expose
/// confidence (or where confidence isn't meaningful) leave it as
/// `None`; single-language detectors that don't track per-region
/// information leave `span` as `None`.
///
/// The `provenance` field records whether this answer came from a
/// real detector run or was asserted by the caller; backends only
/// ever produce [`LanguageProvenance::Detected`], with `Asserted`
/// reserved for the engine when bypassing detection.
#[derive(Debug, Clone)]
pub struct LanguageDetection {
    /// The detected language.
    pub language: LanguageTag,
    /// Optional confidence score. `None` when the backend doesn't
    /// expose one.
    pub confidence: Option<Confidence>,
    /// How this language was obtained: detected or caller-asserted.
    pub provenance: LanguageProvenance,
    /// Byte-offset range this detection applies to, when the backend
    /// reports per-region detections. Single-language detectors that
    /// answer "the whole text is X" leave this `None`.
    pub span: Option<LanguageSpan>,
}

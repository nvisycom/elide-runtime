//! [`LanguageDetection`] — single language-detection result with
//! [`LanguageProvenance`] distinguishing detected from
//! caller-asserted answers, plus the [`LanguageSpan`] byte-offset
//! range it applies to when the detector reports per-region results.

use serde::{Deserialize, Serialize};

use super::LanguageTag;
use crate::primitive::Confidence;

/// Provenance of a [`LanguageDetection`].
///
/// Lets consumers distinguish "the engine ran a detector and got
/// this answer" from "the caller asserted this language and bypassed
/// detection" — without overloading `confidence: None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LanguageProvenance {
    /// Produced by a language-detection backend.
    Detected,
    /// Asserted by the caller, bypassing detection.
    Asserted,
}

/// A byte-offset range within the analyzed text.
///
/// Attached to a [`LanguageDetection`] when the detector knows the
/// span its answer covers (mixed-language input produces multiple
/// detections, each with a distinct span). Single-language
/// detections from non-segmenting backends, and caller-asserted
/// answers, typically leave the span as `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize)]
pub struct LanguageSpan {
    /// Byte offset of the span start in the original text.
    pub start: usize,
    /// Byte offset of the span end in the original text.
    pub end: usize,
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
/// reserved for callers that bypass detection.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageDetection {
    /// The detected language.
    pub language: LanguageTag,
    /// Optional confidence score. `None` when the backend doesn't
    /// expose one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,
    /// How this language was obtained: detected or caller-asserted.
    pub provenance: LanguageProvenance,
    /// Byte-offset range this detection applies to, when the backend
    /// reports per-region detections. Single-language detectors that
    /// answer "the whole text is X" leave this `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<LanguageSpan>,
}

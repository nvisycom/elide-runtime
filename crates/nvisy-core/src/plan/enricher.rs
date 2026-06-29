//! Enricher params: per-kind slots inside an
//! [`AnalyzerParams`].
//!
//! Three enricher kinds — language detection, OCR, STT — each
//! at-most-one per analyzer. Enrichers run sequentially before
//! recognition; each writes to the per-call working context so
//! recognizers downstream see what it wrote.
//!
//! [`AnalyzerParams`]: super::AnalyzerParams

use elide_core::primitive::LanguageTag;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Enricher slots an analyzer can fill. Each slot is
/// at-most-one; the slot name is the kind.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnricherParams {
    /// Detect the document's primary language(s) and write them
    /// into the recognizer context. Drives jurisdiction-aware
    /// recognizer dispatch downstream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<LanguageEnricherParams>,
    /// OCR the image (or PDF page raster) and stamp the
    /// recognised [`Layout`] onto the recognizer context, so
    /// downstream text recognizers can match on the OCR'd text.
    ///
    /// Image modality only.
    ///
    /// [`Layout`]: elide_core::modality::image::Layout
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr: Option<OcrEnricherParams>,
    /// Speech-to-text: transcribe the audio stream and stamp
    /// the resulting [`TranscriptSegment`]s onto the recognizer
    /// context, so downstream text recognizers can match
    /// against the transcript.
    ///
    /// Audio modality only.
    ///
    /// [`TranscriptSegment`]: elide_core::modality::audio::TranscriptSegment
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stt: Option<SttEnricherParams>,
}

/// Params for the language-detection enricher.
///
/// Backed by [`elide_lingua::LinguaEnricher`]: the `candidates` set
/// scopes the detector to a candidate language pool. An empty list
/// asks lingua to consider every language compiled into its
/// feature set.
///
/// [`elide_lingua::LinguaEnricher`]: https://docs.rs/elide-lingua/latest/elide_lingua/struct.LinguaEnricher.html
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LanguageEnricherParams {
    /// Candidate language pool the detector picks from. Empty means
    /// "every language lingua was compiled with"; a non-empty list
    /// restricts detection to those languages and is a real
    /// speed + accuracy win when the caller knows what languages
    /// their documents are in.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<LanguageTag>,
}

/// Params for the OCR enricher (image modality).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OcrEnricherParams {
    /// OCR backend choice.
    pub backend: OcrBackendParams,
}

/// How to instantiate the OCR backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OcrBackendParams {
    /// No-op backend; recognises no blocks. For tests, offline
    /// wiring, or skeleton runs.
    Mock,
    /// BentoML-hosted OCR service. Engine wires the shared
    /// `elide-bento` client; per-request URL + model come from
    /// this variant.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
}

/// Params for the STT enricher (audio modality).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SttEnricherParams {
    /// STT backend choice.
    pub backend: SttBackendParams,
}

/// How to instantiate the STT backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SttBackendParams {
    /// No-op backend; emits no transcript segments. For tests
    /// and skeleton runs.
    Mock,
    /// BentoML-hosted STT service. Per-request URL + model come
    /// from this variant. Engine wiring lands when
    /// `elide-bento` ships a `BentoStt` client.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
}

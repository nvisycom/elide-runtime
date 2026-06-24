//! Per-enricher specs.
//!
//! Enrichers run sequentially before recognition; each writes to the
//! per-call working context (asserted languages, OCR layout,
//! transcripts, exclusions, hints) so recognizers downstream see
//! what they wrote.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One enricher to instantiate inside the request's analyzer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EnricherSpec {
    /// Detect the document's primary language(s) and write them
    /// into the recognizer context. Drives jurisdiction-aware
    /// recognizer dispatch downstream.
    Language(LanguageEnricherSpec),
    /// OCR the image (or PDF page raster) and stamp the recognised
    /// [`Layout`] onto the recognizer context, so downstream text
    /// recognizers can match on the OCR'd text.
    ///
    /// Image modality only.
    ///
    /// [`Layout`]: elide_core::modality::image::Layout
    Ocr(OcrEnricherSpec),
}

/// Spec for the language-detection enricher.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LanguageEnricherSpec {
    /// Minimum confidence the detector must report before a
    /// language is asserted into the context. Lower values write
    /// more languages; higher values are stricter.
    ///
    /// `None` lets the engine pick the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_confidence: Option<u8>,
}

/// Spec for the OCR enricher (image modality).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OcrEnricherSpec {
    /// OCR backend choice.
    pub backend: OcrBackendSpec,
}

/// How to instantiate the OCR backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OcrBackendSpec {
    /// No-op backend; recognises no blocks. For tests, offline
    /// wiring, or skeleton runs.
    Mock,
    /// BentoML-hosted OCR service. Engine wires the shared
    /// `elide-bento` client; per-request URL + model come from this
    /// variant.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
}

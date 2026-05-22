//! Extraction node configuration.
//!
//! [`Extraction`] runs at **phase 1**, after ingestion. It converts raw
//! binary content into structured text that downstream detection nodes
//! can operate on. Each modality (visual, audial, text) has its own
//! optional settings; `None` means the modality uses default settings.
//!
//! All applicable modalities always run — the user controls *how*
//! they run, not *whether* they run.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Visual extraction settings (OCR on images and scanned documents).
///
/// Controls the optional secondary passes that run after the core
/// OCR step.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct VisualExtraction {
    /// Run a secondary LLM verification pass on OCR results.
    #[serde(default)]
    pub verification: bool,
    /// Run computer vision entity detection on images.
    #[serde(default)]
    pub entity_detection: bool,
}

/// Audial extraction settings (speech-to-text on audio).
///
/// Controls optional enrichment applied to the base STT transcript.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AudialExtraction {
    /// Segment the audio by speaker identity.
    #[serde(default)]
    pub diarization: bool,
}

/// Text extraction settings for already-text documents.
///
/// Controls optional normalization applied during extraction.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct TextExtraction {
    /// Normalize whitespace (collapse runs, trim lines).
    #[serde(default)]
    pub normalize_whitespace: bool,
}

/// Unified extraction configuration.
///
/// Each modality is optional — `None` means the modality uses
/// default settings. All applicable modalities always run based
/// on the document's content type; settings here customize *how*
/// they run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Extraction {
    /// Visual extraction settings (OCR). `None` = defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual: Option<VisualExtraction>,
    /// Audial extraction settings (STT). `None` = defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audial: Option<AudialExtraction>,
    /// Text extraction settings. `None` = defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<TextExtraction>,
}

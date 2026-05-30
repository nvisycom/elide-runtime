//! Extraction node configuration.
//!
//! [`Extraction`] runs at **phase 1**, after ingestion. It converts raw
//! binary content into structured text that downstream detection nodes
//! can operate on. All applicable modalities always run — the user
//! controls *how* they run, not *whether* they run.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Audial extraction settings (speech-to-text on audio).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AudialExtraction {
    /// Segment the audio by speaker identity.
    #[serde(default)]
    pub diarization: bool,
}

/// Unified extraction configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Extraction {
    /// Audial extraction settings (STT). `None` = defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audial: Option<AudialExtraction>,
}

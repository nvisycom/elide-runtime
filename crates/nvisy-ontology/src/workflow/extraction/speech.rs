//! Audial extraction node configuration.
//!
//! [`AudialExtraction`] runs at **phase 1**, after ingestion. It transcribes
//! speech audio into text using automatic speech recognition, with an optional
//! speaker diarization pass to segment the transcript by speaker identity.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Configuration for the [`AudialExtraction`] graph node.
///
/// Controls optional enrichment applied to the base speech-to-text transcript.
///
/// [`AudialExtraction`]: crate::graph::GraphNodeKind::AudialExtraction
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AudialExtraction {
    /// Segment the audio by speaker identity.
    #[serde(default)]
    pub diarization: bool,
}

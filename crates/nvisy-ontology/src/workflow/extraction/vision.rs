//! Visual extraction node configuration.
//!
//! [`VisualExtraction`] runs at **phase 1**, after ingestion. It extracts
//! text and structural information from images and scanned documents using
//! optical character recognition, with optional LLM verification and
//! computer-vision entity detection passes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Configuration for the [`VisualExtraction`] graph node.
///
/// Controls the optional secondary passes that run after the core OCR step.
///
/// [`VisualExtraction`]: crate::graph::GraphNodeKind::VisualExtraction
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

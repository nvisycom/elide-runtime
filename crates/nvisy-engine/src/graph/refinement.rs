//! Refinement action configurations: fusion, redaction, and validation.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Configuration for the [`Fusion`] action.
///
/// [`Fusion`]: super::GraphNodeKind::Fusion
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Fusion {
    /// Remove overlapping duplicate entities before fusion.
    #[serde(default)]
    pub entity_deduplication: bool,
    /// Adjust raw model scores to align with empirical precision targets.
    #[serde(default)]
    pub confidence_calibration: bool,
    /// Use surrounding document context to upgrade or downgrade confidence.
    #[serde(default)]
    pub contextual_adjustment: bool,
}

/// Configuration for the [`Redaction`] action.
///
/// [`Redaction`]: super::GraphNodeKind::Redaction
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Redaction {
    /// Strip or redact document metadata (EXIF, PDF properties).
    #[serde(default)]
    pub process_metadata: bool,
}

/// Configuration for the [`Validation`] action.
///
/// [`Validation`]: super::GraphNodeKind::Validation
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Validation {
    /// Fail the run if any leaked values are detected.
    #[serde(default)]
    pub fail_on_leak: bool,
}

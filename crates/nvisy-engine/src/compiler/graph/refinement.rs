//! Refinement action configurations: fusion and redaction.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Configuration for the [`Fusion`](super::GraphNodeKind::Fusion) action.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema
)]
pub struct FusionAction {
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

/// Configuration for the [`Redaction`](super::GraphNodeKind::Redaction) action.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema
)]
pub struct RedactionAction {
    /// Run a validation pass on the redacted output.
    #[serde(default)]
    pub validation: bool,
    /// Strip or redact document metadata (EXIF, PDF properties).
    #[serde(default)]
    pub process_metadata: bool,
}

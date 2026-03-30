//! Pattern recognition node configuration.
//!
//! [`PatternRecognition`] runs at **phase 2**, alongside
//! [`NamedEntityRecognition`]. It detects entities using deterministic
//! rules: regular expressions, checksums, and dictionary lookups.
//!
//! [`NamedEntityRecognition`]: super::NamedEntityRecognition

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Configuration for the [`PatternRecognition`] graph node.
///
/// Controls which patterns run, what confidence threshold applies, and
/// whether co-occurrence context boosting is enabled.
///
/// [`PatternRecognition`]: crate::workflow::GraphNodeKind::PatternRecognition
#[derive(Debug, Clone, Default, PartialEq, Validate)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct PatternRecognition {
    /// Restrict detection to the named patterns only. When empty, all
    /// built-in patterns are used.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patterns: Vec<String>,
    /// Minimum confidence threshold for detections (0.0 to 1.0).
    /// When `None`, the engine's default threshold applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub confidence_threshold: Option<f64>,
    /// Enable co-occurrence keyword analysis for contextual confidence
    /// boosting. When enabled, patterns with context rules receive a
    /// confidence boost if their keywords appear nearby.
    #[serde(default = "default_true")]
    pub contextual_analysis: bool,
}

fn default_true() -> bool {
    true
}

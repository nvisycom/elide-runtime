//! Pattern recognition node configuration.
//!
//! [`PatternRecognition`] runs at **phase 2**, alongside
//! [`NamedEntityRecognition`]. It detects entities using deterministic rules:
//! regular expressions, checksums, dictionary lookups, and structural
//! heuristics, with optional contextual analysis and a second strict pass.
//!
//! [`NamedEntityRecognition`]: crate::graph::NamedEntityRecognition

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Configuration for the [`PatternRecognition`] graph node.
///
/// Each field enables or disables a distinct detection strategy. The default
/// profile enables contextual analysis and a second pass, but leaves
/// heuristic detection off.
///
/// [`PatternRecognition`]: crate::graph::GraphNodeKind::PatternRecognition
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatternRecognition {
    /// Enable format heuristics, entropy, and structural cues.
    #[serde(default)]
    pub heuristic: bool,
    /// Enable co-occurrence analysis for contextual confidence adjustment.
    #[serde(default = "default_true")]
    pub contextual_analysis: bool,
    /// Run a second pass with stricter thresholds.
    #[serde(default = "default_true")]
    pub second_pass: bool,
}

impl Default for PatternRecognition {
    fn default() -> Self {
        Self {
            heuristic: false,
            contextual_analysis: true,
            second_pass: true,
        }
    }
}

fn default_true() -> bool {
    true
}

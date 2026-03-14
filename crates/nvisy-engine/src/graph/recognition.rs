//! Recognition action configurations: NER and pattern-based.

use nvisy_core::Error;
use nvisy_ontology::entity::EntityKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Configuration for the [`NamedEntityRecognition`](super::GraphNodeKind::NamedEntityRecognition) action.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NamedEntityRecognition {
    /// Entity kinds to detect. An empty list means all known kinds.
    #[serde(default)]
    pub entity_kinds: Vec<EntityKind>,
    /// Minimum confidence threshold for detections (0.0 to 1.0).
    /// When `None`, confidence filtering is disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_threshold: Option<f64>,
}

impl NamedEntityRecognition {
    /// Validates that the confidence threshold, if set, is within `0.0..=1.0`.
    pub fn validate(&self) -> Result<(), Error> {
        if let Some(t) = self.confidence_threshold {
            if !(0.0..=1.0).contains(&t) {
                return Err(Error::validation(
                    format!(
                        "confidence_threshold must be between 0.0 and 1.0, got {}",
                        t,
                    ),
                    "compiler",
                ));
            }
        }
        Ok(())
    }
}

/// Configuration for the [`PatternRecognition`](super::GraphNodeKind::PatternRecognition) action.
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

//! Recognition action configurations: NER and pattern-based.

use nvisy_core::Error;
use nvisy_ontology::entity::EntityKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Default minimum confidence threshold for NER detections.
const DEFAULT_CONFIDENCE_THRESHOLD: f64 = 0.5;

/// Configuration for the [`NamedEntityRecognition`](super::GraphNodeKind::NamedEntityRecognition) action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NamedEntityRecognitionAction {
    /// Entity kinds to detect. An empty list means all known kinds.
    #[serde(default)]
    pub entity_kinds: Vec<EntityKind>,
    /// Minimum confidence threshold for detections (0.0 to 1.0).
    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: f64,
}

impl Default for NamedEntityRecognitionAction {
    fn default() -> Self {
        Self {
            entity_kinds: Vec::new(),
            confidence_threshold: DEFAULT_CONFIDENCE_THRESHOLD,
        }
    }
}

impl NamedEntityRecognitionAction {
    /// Validates that the confidence threshold is within `0.0..=1.0`.
    pub fn validate(&self) -> Result<(), Error> {
        if !(0.0..=1.0).contains(&self.confidence_threshold) {
            return Err(Error::validation(
                format!(
                    "confidence_threshold must be between 0.0 and 1.0, got {}",
                    self.confidence_threshold,
                ),
                "compiler",
            ));
        }
        Ok(())
    }
}

/// Configuration for the [`PatternRecognition`](super::GraphNodeKind::PatternRecognition) action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatternRecognitionAction {
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

impl Default for PatternRecognitionAction {
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

fn default_confidence_threshold() -> f64 {
    DEFAULT_CONFIDENCE_THRESHOLD
}

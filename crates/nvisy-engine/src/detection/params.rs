//! [`DetectionParams`]: cross-recognizer per-call hints honored by
//! every built-in recognizer.

use nvisy_ontology::entity::EntityKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Cross-recognizer hints applied to every detection call.
///
/// `entity_kinds` and `confidence_threshold` are honored by every
/// built-in recognizer (NER, pattern, LLM). They live here — not
/// on any per-recognizer config — because they aren't specific to
/// any one backend: the workflow says "I want these kinds, above
/// this confidence" and every recognizer in the engine applies the
/// constraint.
#[derive(Debug, Clone, Default, PartialEq, Validate)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DetectionParams {
    /// Entity kinds to detect. An empty list means all known kinds.
    #[serde(default)]
    pub entity_kinds: Vec<EntityKind>,
    /// Minimum confidence threshold for detections (0.0 to 1.0).
    /// When `None`, confidence filtering is disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub confidence_threshold: Option<f64>,
}

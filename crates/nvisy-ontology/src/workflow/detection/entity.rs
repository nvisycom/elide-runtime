//! Named entity recognition node configuration.
//!
//! [`NamedEntityRecognition`] runs at **phase 2**, after extraction. It drives
//! language-model inference to identify and classify named entities within the
//! extracted text, optionally filtering by entity kind and confidence score.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ValidationError;
use crate::entity::EntityKind;

/// Configuration for the [`NamedEntityRecognition`] graph node.
///
/// Controls which entity kinds are targeted and sets the minimum confidence
/// threshold below which detections are discarded.
///
/// [`NamedEntityRecognition`]: crate::graph::GraphNodeKind::NamedEntityRecognition
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
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(t) = self.confidence_threshold
            && !(0.0..=1.0).contains(&t)
        {
            return Err(ValidationError::new(format!(
                "confidence_threshold must be between 0.0 and 1.0, got {t}"
            )));
        }
        Ok(())
    }
}

//! Shared detection configuration used by NER + CV agents.
//!
//! Lives in [`base`] because both [`NerAgent`] and [`CvAgent`]
//! consume it; the type isn't detection-specific to one agent kind.
//!
//! [`base`]: super
//! [`NerAgent`]: crate::agent::ner::NerAgent
//! [`CvAgent`]: crate::agent::cv::CvAgent

use nvisy_ontology::entity::EntityKind;
use uuid::Uuid;

/// Fallback hint used in prompts when no specific entity types are requested.
pub(crate) const ALL_TYPES_HINT: &str = "all entity types";

/// Configuration for entity detection: which types to look for and at
/// what confidence threshold.
#[derive(Debug, Clone, Default)]
pub struct LlmNerContext {
    /// Entity kinds to detect (empty = all).
    pub entity_kinds: Vec<EntityKind>,
    /// Minimum confidence score to include a detection (0.0..=1.0).
    /// When `None`, no confidence filtering is applied.
    pub confidence_threshold: Option<f64>,
    /// System prompt override (if set, replaces the agent's default).
    pub system_prompt: Option<String>,
    /// Correlation UUID propagated through the tracing span. Not
    /// used for detection.
    pub correlation_id: Option<Uuid>,
}

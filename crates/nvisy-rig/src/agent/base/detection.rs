//! Detection configuration, request, and response types.
//!
//! These were originally in `backend/mod.rs` but belong here because they
//! are agent-specific: every consumer that needs a [`DetectionConfig`] is
//! an agent or an agent prompt builder.

use nvisy_ontology::entity::EntityKind;

/// Fallback hint used in prompts when no specific entity types are requested.
pub(crate) const ALL_TYPES_HINT: &str = "all entity types";

/// Configuration for entity detection: which types to look for and at what
/// confidence threshold.
#[derive(Debug, Clone)]
pub struct DetectionConfig {
    /// Entity kinds to detect (empty = all).
    pub entity_kinds: Vec<EntityKind>,
    /// Minimum confidence score to include a detection (0.0..=1.0).
    pub confidence_threshold: f64,
    /// System prompt override (if set, replaces the agent's default).
    pub system_prompt: Option<String>,
}

/// Request payload for the detection service.
#[derive(Debug, Clone)]
pub struct DetectionRequest {
    pub text: String,
    pub config: DetectionConfig,
}

//! LLM backend types, error mapping, and Tower retry policy.

mod error;
mod metrics;
mod retry;

pub(crate) use error::{from_completion, from_prompt};
pub use metrics::{UsageStats, UsageTracker};
pub use retry::RetryPolicy;

/// Fallback hint used in prompts when no specific entity types are requested.
pub(crate) const ALL_TYPES_HINT: &str = "all entity types";

use serde_json::Value;

use nvisy_ontology::entity::EntityKind;

/// Configuration passed to a detection backend.
#[derive(Debug, Clone)]
pub struct DetectionConfig {
    /// Entity kinds to detect (empty = all).
    pub entity_kinds: Vec<EntityKind>,
    /// Minimum confidence score to include a detection (0.0..=1.0).
    pub confidence_threshold: f64,
    /// System prompt override (if empty, the backend uses its default).
    pub system_prompt: Option<String>,
}

/// Request type for the Tower-based detection service.
#[derive(Debug, Clone)]
pub struct DetectionRequest {
    pub text: String,
    pub config: DetectionConfig,
}

/// Response type for the Tower-based detection service.
#[derive(Debug, Clone)]
pub struct DetectionResponse {
    pub entities: Vec<Value>,
    pub usage: Option<rig::completion::Usage>,
}

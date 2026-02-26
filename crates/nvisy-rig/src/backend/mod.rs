//! LLM backend: agent infrastructure, provider connections, and usage tracking.

mod agent;
pub(crate) mod context;
mod metrics;
pub(crate) mod provider;

pub(crate) use agent::BaseAgent;
pub use agent::BaseAgentConfig;
pub use context::ContextWindow;
pub use metrics::{UsageStats, UsageTracker};
pub use provider::{AuthenticatedProvider, Provider, UnauthenticatedProvider};

use serde_json::Value;

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

/// Response from the detection service.
#[derive(Debug, Clone)]
pub struct DetectionResponse {
    pub entities: Vec<Value>,
    pub usage: Option<rig::completion::Usage>,
}

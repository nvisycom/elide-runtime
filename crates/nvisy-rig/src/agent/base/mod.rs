//! Internal foundation agent and builder.
//!
//! [`BaseAgent`] wraps rig-core's `Agent<M>` with usage tracking and
//! structured-output fallback. [`BaseAgentBuilder`] handles rig-core's
//! typestate for optional tools.

mod agent;
mod builder;
pub(crate) mod context;

pub(crate) use agent::BaseAgent;
pub(crate) use builder::BaseAgentBuilder;

use context::ContextWindow;

/// Configuration for a [`BaseAgent`].
#[derive(Debug, Clone)]
pub struct BaseAgentConfig {
    /// Sampling temperature (default: 0.1).
    pub temperature: f64,
    /// Maximum output tokens (default: 4096).
    pub max_tokens: u64,
    /// Optional context window for chunking large inputs.
    pub context_window: Option<ContextWindow>,
}

impl Default for BaseAgentConfig {
    fn default() -> Self {
        Self {
            temperature: 0.1,
            max_tokens: 4096,
            context_window: None,
        }
    }
}

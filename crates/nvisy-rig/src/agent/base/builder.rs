//! [`BaseAgentBuilder`]: builder for [`BaseAgent`] handling rig-core's
//! typestate for optional tools.

use rig::tool::{Tool, ToolDyn};
use uuid::Uuid;

use crate::backend::UsageTracker;
use crate::error::Error;

use super::dispatch::Agents;
use super::provider::Provider;
use super::{BaseAgent, BaseAgentConfig};

/// Builder for [`BaseAgent`] that takes a `&Provider` + config.
pub(crate) struct BaseAgentBuilder {
    provider: Provider,
    config: BaseAgentConfig,
    preamble: Option<String>,
    tools: Vec<Box<dyn ToolDyn>>,
}

impl BaseAgentBuilder {
    /// Create a new builder with the given provider and config.
    pub fn new(provider: &Provider, config: BaseAgentConfig) -> Self {
        Self {
            provider: provider.clone(),
            config,
            preamble: None,
            tools: Vec::new(),
        }
    }

    /// Set the system prompt (preamble).
    pub fn preamble(mut self, preamble: impl Into<String>) -> Self {
        self.preamble = Some(preamble.into());
        self
    }

    /// Add a tool to the agent.
    pub fn tool(mut self, tool: impl Tool + 'static) -> Self {
        self.tools.push(Box::new(tool));
        self
    }

    /// Build the [`BaseAgent`].
    pub fn build(self) -> Result<BaseAgent, Error> {
        let Self {
            provider,
            config,
            preamble,
            tools,
        } = self;

        let inner = Agents::build(
            &provider,
            &config,
            preamble.as_deref(),
            tools,
        )?;

        Ok(BaseAgent {
            id: Uuid::now_v7(),
            inner,
            context_window: config.context_window,
            tracker: UsageTracker::new(),
        })
    }
}

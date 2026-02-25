//! [`BaseAgentBuilder`] — builder for [`BaseAgent`] handling rig-core's
//! typestate for optional tools.

use rig::agent::AgentBuilder;
use rig::completion::CompletionModel;
use rig::tool::{Tool, ToolDyn};

use crate::backend::UsageTracker;

use super::{BaseAgent, BaseAgentConfig};

/// Builder for [`BaseAgent`] that handles rig-core's typestate for tools.
pub(crate) struct BaseAgentBuilder<M: CompletionModel> {
    model: M,
    config: BaseAgentConfig,
    preamble: Option<String>,
    tools: Vec<Box<dyn ToolDyn>>,
}

impl<M: CompletionModel> BaseAgentBuilder<M> {
    /// Create a new builder with the given model and config.
    pub fn new(model: M, config: BaseAgentConfig) -> Self {
        Self {
            model,
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
    pub fn build(self) -> BaseAgent<M> {
        let agent = if self.tools.is_empty() {
            let mut builder = AgentBuilder::new(self.model)
                .temperature(self.config.temperature)
                .max_tokens(self.config.max_tokens);

            if let Some(ref preamble) = self.preamble {
                builder = builder.preamble(preamble);
            }

            builder.build()
        } else {
            let mut builder = AgentBuilder::new(self.model)
                .temperature(self.config.temperature)
                .max_tokens(self.config.max_tokens)
                .tools(self.tools);

            if let Some(ref preamble) = self.preamble {
                builder = builder.preamble(preamble);
            }

            builder.build()
        };

        BaseAgent {
            agent,
            context_window: self.config.context_window,
            tracker: UsageTracker::new(),
        }
    }
}

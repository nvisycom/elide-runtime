//! [`BaseAgentBuilder`]: builder for [`BaseAgent`] handling rig-core's
//! typestate for optional tools.

use rig::agent::AgentBuilder;
use rig::client::CompletionClient;
use rig::completion::CompletionModel;
use rig::tool::{Tool, ToolDyn};
use uuid::Uuid;

use crate::backend::UsageTracker;
use crate::error::Error;

use super::dispatch::Agents;
use super::provider::{Provider, ProviderClient};
use super::{BaseAgent, BaseAgentConfig};

/// Builder for [`BaseAgent`] that takes a `&Provider` + model name.
pub(crate) struct BaseAgentBuilder {
    provider: Provider,
    model_name: String,
    config: BaseAgentConfig,
    preamble: Option<String>,
    tools: Vec<Box<dyn ToolDyn>>,
}

impl BaseAgentBuilder {
    /// Create a new builder with the given provider, model name, and config.
    pub fn new(provider: &Provider, model_name: &str, config: BaseAgentConfig) -> Self {
        Self {
            provider: provider.clone(),
            model_name: model_name.to_owned(),
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
            model_name,
            config,
            preamble,
            tools,
        } = self;

        let preamble_ref = preamble.as_deref();
        let client = ProviderClient::from_provider(&provider)?;

        let inner = match client {
            ProviderClient::OpenAi(c) => {
                Agents::OpenAi(build_rig_agent(c.completion_model(&model_name), &config, preamble_ref, tools))
            }
            ProviderClient::Anthropic(c) => {
                Agents::Anthropic(build_rig_agent(c.completion_model(&model_name), &config, preamble_ref, tools))
            }
            ProviderClient::Gemini(c) => {
                Agents::Gemini(build_rig_agent(c.completion_model(&model_name), &config, preamble_ref, tools))
            }
            ProviderClient::Ollama(c) => {
                Agents::Ollama(build_rig_agent(c.completion_model(&model_name), &config, preamble_ref, tools))
            }
        };

        Ok(BaseAgent {
            id: Uuid::now_v7(),
            inner,
            context_window: config.context_window,
            tracker: UsageTracker::new(),
        })
    }
}

/// Build a concrete rig-core `Agent<M>`.
///
/// Generic over `M` but only called inside [`BaseAgentBuilder::build`] —
/// the generic never escapes the module boundary.
fn build_rig_agent<M: CompletionModel>(
    model: M,
    config: &BaseAgentConfig,
    preamble: Option<&str>,
    tools: Vec<Box<dyn ToolDyn>>,
) -> rig::agent::Agent<M> {
    if tools.is_empty() {
        let mut builder = AgentBuilder::new(model)
            .temperature(config.temperature)
            .max_tokens(config.max_tokens);

        if let Some(preamble) = preamble {
            builder = builder.preamble(preamble);
        }

        builder.build()
    } else {
        let mut builder = AgentBuilder::new(model)
            .temperature(config.temperature)
            .max_tokens(config.max_tokens)
            .tools(tools);

        if let Some(preamble) = preamble {
            builder = builder.preamble(preamble);
        }

        builder.build()
    }
}

//! Builder for [`BaseAgent`](super::BaseAgent).

use reqwest_middleware::ClientWithMiddleware;
use rig::agent::{Agent, AgentBuilder};
use rig::client::CompletionClient;
use rig::completion::CompletionModel;
use rig::providers::{anthropic, gemini, ollama, openai};
use rig::tool::{Tool, ToolDyn};
use uuid::Uuid;

use super::super::provider::{Provider, build_http_client};
use super::super::UsageTracker;
use super::{Agents, BaseAgent, BaseAgentConfig};
use crate::error::Error;

/// Builder for [`BaseAgent`].
///
/// Created via [`BaseAgent::builder`]. Collects a provider reference, config,
/// optional preamble (system prompt), and optional tools, then constructs the
/// concrete rig-core agent on [`build`](Self::build).
pub(crate) struct BaseAgentBuilder {
    provider: Provider,
    config: BaseAgentConfig,
    preamble: Option<String>,
    tools: Vec<Box<dyn ToolDyn>>,
}

impl BaseAgentBuilder {
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

    /// Register a tool the agent can call during prompts.
    pub fn tool(mut self, tool: impl Tool + 'static) -> Self {
        self.tools.push(Box::new(tool));
        self
    }

    /// Build the [`BaseAgent`], constructing the provider-specific rig client.
    pub fn build(self) -> Result<BaseAgent, Error> {
        let Self {
            provider,
            config,
            preamble,
            tools,
        } = self;

        let http_client = build_http_client(config.max_retries);
        let preamble = preamble.as_deref();

        let inner = match &provider {
            Provider::OpenAi(p) => {
                let mut b = openai::Client::<ClientWithMiddleware>::builder()
                    .api_key(&p.api_key)
                    .http_client(http_client);
                if let Some(url) = &p.base_url {
                    b = b.base_url(url);
                }
                let client = b.build().map_err(|e| Error::Client(e.to_string()))?;
                let model = client.completions_api().completion_model(&p.model);
                Agents::OpenAi(build_rig_agent(model, &config, preamble, tools))
            }
            Provider::Anthropic(p) => {
                let mut b = anthropic::Client::<ClientWithMiddleware>::builder()
                    .api_key(&p.api_key)
                    .http_client(http_client);
                if let Some(url) = &p.base_url {
                    b = b.base_url(url);
                }
                let client = b.build().map_err(|e| Error::Client(e.to_string()))?;
                let model = client.completion_model(&p.model);
                Agents::Anthropic(build_rig_agent(model, &config, preamble, tools))
            }
            Provider::Gemini(p) => {
                let mut b = gemini::Client::<ClientWithMiddleware>::builder()
                    .api_key(&p.api_key)
                    .http_client(http_client);
                if let Some(url) = &p.base_url {
                    b = b.base_url(url);
                }
                let client = b.build().map_err(|e| Error::Client(e.to_string()))?;
                // rig-core 0.31: Gemini's Capabilities doesn't propagate H,
                // so CompletionClient is unavailable for non-default H.
                let model = gemini::completion::CompletionModel::new(client, &p.model);
                Agents::Gemini(build_rig_agent(model, &config, preamble, tools))
            }
            Provider::Ollama(p) => {
                let mut b = ollama::Client::<ClientWithMiddleware>::builder()
                    .api_key(rig::client::Nothing)
                    .http_client(http_client);
                if let Some(url) = &p.base_url {
                    b = b.base_url(url);
                }
                let client = b.build().map_err(|e| Error::Client(e.to_string()))?;
                let model = client.completion_model(&p.model);
                Agents::Ollama(build_rig_agent(model, &config, preamble, tools))
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
) -> Agent<M> {
    // AgentBuilder uses typestate: `.tools()` changes the type parameter,
    // so the with-tools and without-tools paths cannot share a binding.
    if tools.is_empty() {
        let mut b = AgentBuilder::new(model)
            .temperature(config.temperature)
            .max_tokens(config.max_tokens);
        if let Some(p) = preamble {
            b = b.preamble(p);
        }
        b.build()
    } else {
        let mut b = AgentBuilder::new(model)
            .temperature(config.temperature)
            .max_tokens(config.max_tokens)
            .tools(tools);
        if let Some(p) = preamble {
            b = b.preamble(p);
        }
        b.build()
    }
}

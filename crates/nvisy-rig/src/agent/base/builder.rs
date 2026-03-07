//! Builder for [`BaseAgent`](super::BaseAgent).

use nvisy_http::{HttpClient, HttpConfig};
use rig::agent::{Agent, AgentBuilder};
use rig::client::CompletionClient;
use rig::completion::CompletionModel;
#[cfg(feature = "google-gemini")]
use rig::providers::gemini;
use rig::tool::{Tool, ToolDyn};
use uuid::Uuid;

use super::{AgentConfig, AgentProvider, Agents, BaseAgent};
use crate::backend::UsageTracker;
use crate::error::Error;

/// Builder for [`BaseAgent`].
///
/// Created via [`BaseAgent::builder`]. Collects a provider reference, config,
/// and optional tools, then constructs the concrete rig-core agent on
/// [`build`](Self::build).
pub(crate) struct BaseAgentBuilder {
    provider: AgentProvider,
    config: AgentConfig,
    tools: Vec<Box<dyn ToolDyn>>,
    http_client: Option<HttpClient>,
}

impl BaseAgentBuilder {
    pub fn new(provider: &AgentProvider, config: AgentConfig) -> Self {
        Self {
            provider: provider.clone(),
            config,
            tools: Vec::new(),
            http_client: None,
        }
    }

    /// Register a tool the agent can call during prompts.
    pub fn tool(mut self, tool: impl Tool + 'static) -> Self {
        self.tools.push(Box::new(tool));
        self
    }

    /// Use a pre-built HTTP client instead of constructing a new one.
    pub fn http_client(mut self, client: HttpClient) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Build the [`BaseAgent`], constructing the provider-specific rig client.
    pub fn build(self) -> Result<BaseAgent, Error> {
        let Self {
            provider,
            config,
            tools,
            http_client: existing_client,
        } = self;

        let http_client = existing_client.unwrap_or_else(|| {
            HttpClient::new(&HttpConfig {
                max_retries: config.max_retries,
                ..HttpConfig::default()
            })
        });
        let preamble = config.preamble.as_deref();

        let raw_client = http_client.into_inner();

        let inner = match &provider {
            #[cfg(feature = "openai-gpt")]
            AgentProvider::OpenAi(p) => {
                let client = p.openai_client(raw_client)?;
                let model = client.completions_api().completion_model(&p.model);
                Agents::OpenAi(build_rig_agent(model, &config, preamble, tools))
            }
            #[cfg(feature = "anthropic-claude")]
            AgentProvider::Anthropic(p) => {
                let client = p.anthropic_client(raw_client)?;
                let model = client.completion_model(&p.model);
                Agents::Anthropic(build_rig_agent(model, &config, preamble, tools))
            }
            #[cfg(feature = "google-gemini")]
            AgentProvider::Gemini(p) => {
                let client = p.gemini_client(raw_client)?;
                // rig-core 0.31: Gemini's Capabilities doesn't propagate H,
                // so CompletionClient is unavailable for non-default H.
                let model = gemini::completion::CompletionModel::new(client, &p.model);
                Agents::Gemini(build_rig_agent(model, &config, preamble, tools))
            }
            AgentProvider::Ollama(p) => {
                let client = p.ollama_client(raw_client)?;
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
    config: &AgentConfig,
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

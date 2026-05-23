//! Builder for [`BaseAgent`].
//!
//! [`BaseAgent`]: super::BaseAgent

use nvisy_core::http::{HttpClient, HttpConfig};
use rig::agent::{Agent, AgentBuilder};
use rig::client::CompletionClient;
use rig::completion::CompletionModel;
#[cfg(feature = "google-gemini")]
use rig::providers::gemini;
use uuid::Uuid;

use super::{AgentConfig, AgentProvider, Agents, BaseAgent, UsageTracker};
use crate::error::Error;

/// Builder for [`BaseAgent`].
///
/// Created via [`BaseAgent::builder`]. Collects a provider reference
/// and config, then constructs the concrete rig-core agent on
/// [`build`].
///
/// [`build`]: Self::build
pub(crate) struct BaseAgentBuilder {
    provider: AgentProvider,
    config: AgentConfig,
}

impl BaseAgentBuilder {
    pub fn new(provider: &AgentProvider, config: AgentConfig) -> Self {
        Self {
            provider: provider.clone(),
            config,
        }
    }

    /// Build the [`BaseAgent`], constructing the provider-specific rig client.
    pub fn build(self) -> Result<BaseAgent, Error> {
        let Self { provider, config } = self;

        let http_client = HttpClient::new(&HttpConfig {
            max_retries: config.max_retries,
            ..HttpConfig::default()
        })
        .map_err(|e| Error::Request(e.to_string()))?;
        let preamble = config.preamble.as_deref();

        let raw_client = http_client.into_inner();

        let inner = match &provider {
            #[cfg(feature = "openai-gpt")]
            AgentProvider::OpenAi(p) => {
                let client = p.openai_client(raw_client)?;
                let model = client.completions_api().completion_model(&p.model);
                Agents::OpenAi(build_rig_agent(model, &config, preamble))
            }
            #[cfg(feature = "anthropic-claude")]
            AgentProvider::Anthropic(p) => {
                let client = p.anthropic_client(raw_client)?;
                let model = client.completion_model(&p.model);
                Agents::Anthropic(build_rig_agent(model, &config, preamble))
            }
            #[cfg(feature = "google-gemini")]
            AgentProvider::Gemini(p) => {
                let client = p.gemini_client(raw_client)?;
                // rig-core 0.31: Gemini's Capabilities doesn't propagate H,
                // so CompletionClient is unavailable for non-default H.
                let model = gemini::completion::CompletionModel::new(client, &p.model);
                Agents::Gemini(build_rig_agent(model, &config, preamble))
            }
            AgentProvider::Ollama(p) => {
                let client = p.ollama_client(raw_client)?;
                let model = client.completion_model(&p.model);
                Agents::Ollama(build_rig_agent(model, &config, preamble))
            }
        };

        Ok(BaseAgent {
            id: Uuid::now_v7(),
            inner,
            context_window: config.context_window,
            tracker: UsageTracker::new(),
            model_name: provider.model().to_owned(),
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
) -> Agent<M> {
    let mut b = AgentBuilder::new(model)
        .temperature(config.temperature)
        .max_tokens(config.max_tokens);
    if let Some(p) = preamble {
        b = b.preamble(p);
    }
    b.build()
}

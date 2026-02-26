//! Provider-specific agent variants.
//!
//! [`Agents`] wraps a concrete `rig::agent::Agent<M>` for each
//! supported provider, enabling dispatch without exposing `CompletionModel`
//! generics to the public API.

use reqwest_middleware::ClientWithMiddleware;
use rig::agent::{Agent, AgentBuilder};
use rig::client::CompletionClient;
use rig::completion::CompletionModel;
use rig::providers::{anthropic, gemini, ollama, openai};
use rig::tool::ToolDyn;

use crate::error::Error;

use super::BaseAgentConfig;
use super::provider::{Provider, build_http_client};

pub(crate) enum Agents {
    OpenAi(Agent<openai::completion::CompletionModel<ClientWithMiddleware>>),
    Anthropic(Agent<anthropic::completion::CompletionModel<ClientWithMiddleware>>),
    Gemini(Agent<gemini::completion::CompletionModel<ClientWithMiddleware>>),
    Ollama(Agent<ollama::CompletionModel<ClientWithMiddleware>>),
}

impl Agents {
    /// Build an [`Agents`] variant from provider connection params.
    pub(crate) fn build(
        provider: &Provider,
        config: &BaseAgentConfig,
        preamble: Option<&str>,
        tools: Vec<Box<dyn ToolDyn>>,
    ) -> Result<Self, Error> {
        let http_client = build_http_client(config.max_retries);

        match provider {
            Provider::OpenAi(p) => {
                let mut builder = openai::Client::<ClientWithMiddleware>::builder()
                    .api_key(&p.api_key)
                    .http_client(http_client);
                if let Some(url) = &p.base_url {
                    builder = builder.base_url(url);
                }
                let client = builder.build().map_err(|e| Error::Client(e.to_string()))?;
                let model = client.completions_api().completion_model(&p.model);
                Ok(Self::OpenAi(build_rig_agent(model, config, preamble, tools)))
            }
            Provider::Anthropic(p) => {
                let mut builder = anthropic::Client::<ClientWithMiddleware>::builder()
                    .api_key(&p.api_key)
                    .http_client(http_client);
                if let Some(url) = &p.base_url {
                    builder = builder.base_url(url);
                }
                let client = builder.build().map_err(|e| Error::Client(e.to_string()))?;
                let model = client.completion_model(&p.model);
                Ok(Self::Anthropic(build_rig_agent(model, config, preamble, tools)))
            }
            Provider::Gemini(p) => {
                let mut builder = gemini::Client::<ClientWithMiddleware>::builder()
                    .api_key(&p.api_key)
                    .http_client(http_client);
                if let Some(url) = &p.base_url {
                    builder = builder.base_url(url);
                }
                let client = builder.build().map_err(|e| Error::Client(e.to_string()))?;
                // rig-core 0.31: Gemini's Capabilities impl doesn't propagate H,
                // so CompletionClient is unavailable for non-default H.
                let model = gemini::completion::CompletionModel::new(client, &p.model);
                Ok(Self::Gemini(build_rig_agent(model, config, preamble, tools)))
            }
            Provider::Ollama(p) => {
                let mut builder = ollama::Client::<ClientWithMiddleware>::builder()
                    .api_key(rig::client::Nothing)
                    .http_client(http_client);
                if let Some(url) = &p.base_url {
                    builder = builder.base_url(url);
                }
                let client = builder.build().map_err(|e| Error::Client(e.to_string()))?;
                let model = client.completion_model(&p.model);
                Ok(Self::Ollama(build_rig_agent(model, config, preamble, tools)))
            }
        }
    }
}

/// Build a concrete rig-core `Agent<M>`.
///
/// Generic over `M` but only called inside [`Agents::build`]:
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

/// Dispatch a call to the concrete agent inside each variant.
macro_rules! dispatch {
    ($inner:expr, |$agent:ident| $body:expr) => {
        match $inner {
            Agents::OpenAi($agent) => $body,
            Agents::Anthropic($agent) => $body,
            Agents::Gemini($agent) => $body,
            Agents::Ollama($agent) => $body,
        }
    };
}

pub(crate) use dispatch;

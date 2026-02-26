//! Provider-specific agent variants.
//!
//! [`Agents`] wraps a concrete `rig::agent::Agent<M>` for each
//! supported provider, enabling dispatch without exposing `CompletionModel`
//! generics to the public API.

use reqwest_middleware::ClientWithMiddleware;
use rig::agent::Agent;
use rig::providers::{anthropic, gemini, ollama, openai};

pub(crate) enum Agents {
    OpenAi(Agent<openai::completion::CompletionModel<ClientWithMiddleware>>),
    Anthropic(Agent<anthropic::completion::CompletionModel<ClientWithMiddleware>>),
    Gemini(Agent<gemini::completion::CompletionModel<ClientWithMiddleware>>),
    Ollama(Agent<ollama::CompletionModel<ClientWithMiddleware>>),
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

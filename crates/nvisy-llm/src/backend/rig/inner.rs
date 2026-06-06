//! Private rig-agent dispatch enum + macro.
//!
//! Wraps the four provider-specific `rig::Agent<M>` instances behind
//! one enum so the rest of [`super`] can call them uniformly without
//! caring which provider produced the agent.

use reqwest_middleware::ClientWithMiddleware;
use rig::agent::Agent;
#[cfg(feature = "anthropic-claude")]
use rig::providers::anthropic;
#[cfg(feature = "google-gemini")]
use rig::providers::gemini;
use rig::providers::ollama;
#[cfg(feature = "openai-gpt")]
use rig::providers::openai;

pub(super) enum RigInner {
    #[cfg(feature = "openai-gpt")]
    OpenAi(Agent<openai::completion::CompletionModel<ClientWithMiddleware>>),
    #[cfg(feature = "anthropic-claude")]
    Anthropic(Agent<anthropic::completion::CompletionModel<ClientWithMiddleware>>),
    #[cfg(feature = "google-gemini")]
    Gemini(Agent<gemini::completion::CompletionModel<ClientWithMiddleware>>),
    Ollama(Agent<ollama::CompletionModel<ClientWithMiddleware>>),
}

macro_rules! dispatch {
    ($inner:expr, |$agent:ident| $body:expr) => {
        match $inner {
            #[cfg(feature = "openai-gpt")]
            $crate::backend::rig::inner::RigInner::OpenAi($agent) => $body,
            #[cfg(feature = "anthropic-claude")]
            $crate::backend::rig::inner::RigInner::Anthropic($agent) => $body,
            #[cfg(feature = "google-gemini")]
            $crate::backend::rig::inner::RigInner::Gemini($agent) => $body,
            $crate::backend::rig::inner::RigInner::Ollama($agent) => $body,
        }
    };
}

pub(super) use dispatch;

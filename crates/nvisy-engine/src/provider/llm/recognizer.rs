//! [`LlmRecognizer`]: one entry in the deployment's LLM lineup.

use elide::recognition::llm::provider::{AuthenticatedProvider, UnauthenticatedProvider};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{AttachTo, LlmPrompt};

/// One entry in the deployment's LLM lineup.
///
/// Every entry in [`LlmConfig::recognizers`] runs on every
/// analyzer whose modality is listed in [`modalities`], provided
/// the request's `recognizers.llm` selects this recognizer.
///
/// [`LlmConfig::recognizers`]: super::LlmConfig::recognizers
/// [`modalities`]: Self::modalities
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LlmRecognizer {
    /// Recognizer name. Surfaces on the per-entity provenance
    /// trail so audits can attribute detections to a specific
    /// configured recognizer. Must be unique across the
    /// deployment's LLM lineup.
    pub name: String,
    /// Optional human-readable description. Surfaces on the
    /// list-recognizers accessor so operators and SDK callers
    /// can identify what each recognizer is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Source selection + its per-kind fields, flattened onto
    /// the recognizer's wire shape.
    #[serde(flatten)]
    pub source: LlmSource,
    /// Custom prompt source. Omitted means "use elide's default
    /// recognition prompt."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<LlmPrompt>,
    /// Which analyzer modalities this recognizer attaches to.
    /// Empty is an error at compile time: a recognizer that
    /// attaches to no analyzer never runs.
    #[serde(default = "default_modalities")]
    pub modalities: Vec<AttachTo>,
}

/// Where a configured recognizer gets its LLM completions.
///
/// The four rig variants mirror
/// [`elide::recognition::llm::provider::Provider`] and forward its inner
/// payloads directly. The `Mock` variant exists only when the
/// consuming crate enables the `test-utils` feature: the wire
/// rejects `kind = "mock"` in production builds.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LlmSource {
    /// OpenAI GPT provider.
    OpenAi(AuthenticatedProvider),
    /// Anthropic Claude provider.
    Anthropic(AuthenticatedProvider),
    /// Google Gemini provider.
    Gemini(AuthenticatedProvider),
    /// Ollama (local) provider.
    Ollama(UnauthenticatedProvider),
    /// No-op source; emits no entities. Test-only.
    #[cfg(feature = "test-utils")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
    Mock,
}

impl LlmSource {
    /// Provider slug for the list-recognizers accessor.
    #[must_use]
    pub fn provider(&self) -> &'static str {
        match self {
            Self::OpenAi(_) => "openai",
            Self::Anthropic(_) => "anthropic",
            Self::Gemini(_) => "gemini",
            Self::Ollama(_) => "ollama",
            #[cfg(feature = "test-utils")]
            Self::Mock => "mock",
        }
    }
}

fn default_modalities() -> Vec<AttachTo> {
    vec![AttachTo::Text]
}

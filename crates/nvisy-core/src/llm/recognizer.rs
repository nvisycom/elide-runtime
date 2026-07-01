//! [`LlmRecognizer`]: one entry in the deployment's LLM lineup.

use elide_llm::provider::{AuthenticatedProvider, UnauthenticatedProvider};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{LlmPrompt, LlmRecognizerModality};

/// One deployment-configured LLM recognizer. Every entry in
/// [`LlmConfig::recognizers`](super::LlmConfig::recognizers)
/// runs on every analyzer whose modality is listed in
/// [`modalities`](Self::modalities), provided the request
/// toggled `recognizers.llm = true`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LlmRecognizer {
    /// Recognizer name — surfaces on the per-entity provenance
    /// trail so audits can attribute detections to a specific
    /// configured recognizer. Must be unique across the
    /// deployment's LLM lineup.
    pub name: String,
    /// Backend selection + its per-kind fields, flattened onto
    /// the recognizer's wire shape.
    #[serde(flatten)]
    pub backend: LlmBackendConfig,
    /// Custom prompt source. Omitted means "use elide's default
    /// recognition prompt."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<LlmPrompt>,
    /// Which analyzer modalities this recognizer attaches to.
    /// Empty is an error at compile time — a recognizer that
    /// attaches to no analyzer never runs.
    #[serde(default = "default_modalities")]
    pub modalities: Vec<LlmRecognizerModality>,
}

/// How a configured recognizer talks to its model.
///
/// The four rig variants mirror
/// [`elide_llm::provider::Provider`] and forward its inner
/// payloads directly. The `Mock` variant exists only when the
/// consuming crate enables the `test-utils` feature — the wire
/// rejects `kind = "mock"` in production builds.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum LlmBackendConfig {
    /// OpenAI GPT provider.
    OpenAi(AuthenticatedProvider),
    /// Anthropic Claude provider.
    Anthropic(AuthenticatedProvider),
    /// Google Gemini provider.
    Gemini(AuthenticatedProvider),
    /// Ollama (local) provider.
    Ollama(UnauthenticatedProvider),
    /// No-op backend; emits no entities. Test-only.
    #[cfg(feature = "test-utils")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
    Mock,
}

fn default_modalities() -> Vec<LlmRecognizerModality> {
    vec![LlmRecognizerModality::Text]
}

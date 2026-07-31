//! [`RegisteredRecognizer`]: the public view of one recognizer
//! in the engine's NER or LLM lineup.
//!
//! Returned by [`crate::Engine::ner_recognizers`] and
//! [`crate::Engine::llm_recognizers`] so operators and SDK
//! callers can list what's registered without seeing backend
//! connection details or (future) credentials.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::provider::llm::LlmRecognizer;
use crate::provider::ner::NerRecognizer;

/// Public view of one recognizer in the engine's NER or LLM
/// lineup.
///
/// Carries the name a request's allowlist picks by, an optional
/// human-readable description, and a provider slug identifying
/// the backend kind. Connection details and (future)
/// credentials stay in the private `NerConfig` / `LlmConfig`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredRecognizer<'a> {
    /// Recognizer name — the identifier a request's allowlist
    /// picks by.
    pub name: &'a str,
    /// Optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
    /// Provider slug. NER: `"bento"`, `"mock"`. LLM: `"openai"`,
    /// `"anthropic"`, `"gemini"`, `"ollama"`, `"mock"`.
    pub provider: &'static str,
}

impl<'a> From<&'a NerRecognizer> for RegisteredRecognizer<'a> {
    fn from(r: &'a NerRecognizer) -> Self {
        Self {
            name: &r.name,
            description: r.description.as_deref(),
            provider: r.backend.provider(),
        }
    }
}

impl<'a> From<&'a LlmRecognizer> for RegisteredRecognizer<'a> {
    fn from(r: &'a LlmRecognizer) -> Self {
        Self {
            name: &r.name,
            description: r.description.as_deref(),
            provider: r.source.provider(),
        }
    }
}

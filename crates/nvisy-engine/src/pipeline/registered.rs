//! [`RegisteredRecognizer`]: the public view of one recognizer
//! in the engine's NER or LLM lineup.
//!
//! Returned by [`crate::Engine::ner_recognizers`] and
//! [`crate::Engine::llm_recognizers`] so operators and SDK
//! callers can list what's registered without seeing backend
//! connection details or (future) credentials.

use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::provider::llm::LlmRecognizerConfig;
use crate::provider::ner::NerRecognizerConfig;

/// Public view of one recognizer in the engine's NER or LLM
/// lineup.
///
/// Carries the name a request's allowlist picks by, an optional
/// human-readable description, and a provider slug identifying
/// the backend kind. Connection details and (future)
/// credentials stay in the private `NerConfig` / `LlmConfig`.
///
/// Owned rather than borrowing from the engine so callers can
/// carry the value past the borrow that produced it. Cloning is
/// cheap — [`HipStr`] shares the backing string via an `Arc`
/// header.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredRecognizer {
    /// Recognizer name — the identifier a request's allowlist
    /// picks by.
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub description: Option<HipStr<'static>>,
    /// Provider slug. NER: `"bento"`, `"mock"`. LLM: `"openai"`,
    /// `"anthropic"`, `"gemini"`, `"ollama"`, `"mock"`.
    pub provider: &'static str,
}

impl From<&NerRecognizerConfig> for RegisteredRecognizer {
    fn from(r: &NerRecognizerConfig) -> Self {
        Self {
            name: r.name.clone(),
            description: r.description.clone(),
            provider: r.backend.provider(),
        }
    }
}

impl From<&LlmRecognizerConfig> for RegisteredRecognizer {
    fn from(r: &LlmRecognizerConfig) -> Self {
        Self {
            name: r.name.clone(),
            description: r.description.clone(),
            provider: r.source.provider(),
        }
    }
}

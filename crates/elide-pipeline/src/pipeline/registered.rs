//! [`RegisteredRecognizer`]: the public view of one recognizer
//! in the engine's NER or LLM lineup.
//!
//! Carried by [`RegisteredComponents`], which
//! [`crate::Engine::components`] returns so operators and SDK
//! callers can list what's registered without seeing backend
//! connection details or (future) credentials.
//!
//! [`RegisteredEnricher`] is a type alias for the same shape,
//! used for the OCR and STT lineups.

use elide_provider::{
    LlmRecognizerConfig, NerRecognizerConfig, OcrEnricherConfig, SttEnricherConfig,
};
use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
/// cheap: [`HipStr`] shares the backing string via an `Arc`
/// header.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredRecognizer {
    /// Recognizer name: the identifier a request's allowlist
    /// picks by.
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub description: Option<HipStr<'static>>,
    /// Provider slug. NER: `"bento"`, `"mock"`. LLM: `"openai"`,
    /// `"anthropic"`, `"gemini"`, `"ollama"`, `"mock"`.
    ///
    /// Owned so the type deserializes from a runtime buffer: a
    /// `&'static str` field would make the derive emit
    /// `Deserialize<'static>` only, which compiles against string
    /// literals but not against an owned `String` or a reader -
    /// the shapes a host actually decodes from. Borrowing a
    /// `&'static str` into a [`HipStr`] does not allocate, so
    /// engine-side construction stays free.
    #[schemars(with = "String")]
    pub provider: HipStr<'static>,
}

impl From<&NerRecognizerConfig> for RegisteredRecognizer {
    fn from(r: &NerRecognizerConfig) -> Self {
        Self {
            name: r.name.clone(),
            description: r.description.clone(),
            provider: HipStr::from(r.backend.provider()),
        }
    }
}

impl From<&LlmRecognizerConfig> for RegisteredRecognizer {
    fn from(r: &LlmRecognizerConfig) -> Self {
        Self {
            name: r.name.clone(),
            description: r.description.clone(),
            provider: HipStr::from(r.source.provider()),
        }
    }
}

/// Public view of one enricher in the engine's OCR or STT
/// lineup. Same shape as [`RegisteredRecognizer`] because both
/// carry a name, an optional description, and a provider slug.
pub type RegisteredEnricher = RegisteredRecognizer;

impl From<&OcrEnricherConfig> for RegisteredRecognizer {
    fn from(e: &OcrEnricherConfig) -> Self {
        Self {
            name: e.name.clone(),
            description: e.description.clone(),
            provider: HipStr::from(e.backend.provider()),
        }
    }
}

impl From<&SttEnricherConfig> for RegisteredRecognizer {
    fn from(e: &SttEnricherConfig) -> Self {
        Self {
            name: e.name.clone(),
            description: e.description.clone(),
            provider: HipStr::from(e.backend.provider()),
        }
    }
}

/// Every recognizer and enricher an [`Engine`] has registered,
/// each lineup in configuration order.
///
/// One value rather than four separate accessors: a caller
/// listing what an engine can do wants the whole picture, and
/// assembling it from four calls invites showing a partial one.
/// Serializable so a host can return it from an introspection
/// endpoint directly.
///
/// [`Engine`]: crate::Engine
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredComponents {
    /// NER recognizers, in configuration order.
    pub ner: Vec<RegisteredRecognizer>,
    /// LLM recognizers, in configuration order.
    pub llm: Vec<RegisteredRecognizer>,
    /// OCR enrichers, in configuration order.
    pub ocr: Vec<RegisteredEnricher>,
    /// STT enrichers, in configuration order.
    pub stt: Vec<RegisteredEnricher>,
}

impl RegisteredComponents {
    /// Whether the engine has no components registered at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ner.is_empty() && self.llm.is_empty() && self.ocr.is_empty() && self.stt.is_empty()
    }

    /// Total number of registered components across all four
    /// lineups.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ner.len() + self.llm.len() + self.ocr.len() + self.stt.len()
    }
}

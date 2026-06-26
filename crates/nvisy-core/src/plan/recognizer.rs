//! Recognizer params: per-kind slots inside an
//! [`AnalyzerParams`].
//!
//! Three recognizer kinds — pattern, NER, LLM — each with its
//! own slot on [`RecognizerParams`]:
//!
//! - **Pattern** is at-most-one. Single regex/dictionary engine
//!   per analyzer; multi-pattern means accumulating into one
//!   instance's pattern list, not running two engines.
//! - **NER** is a list. Multiple NER backends (different model
//!   versions, language-specialised models, ensemble lanes) run
//!   in parallel; each is identified by [`name`] for
//!   provenance.
//! - **LLM** is a list. Multiple providers (OpenAI, Anthropic,
//!   …) or per-label-class specialist models, identified by
//!   [`name`].
//!
//! [`AnalyzerParams`]: super::AnalyzerParams
//! [`name`]: NerRecognizerParams::name

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Recognizer slots an analyzer can fill. The slot name is the
/// kind; pattern is at-most-one, NER and LLM accept many
/// (identified by `name`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecognizerParams {
    /// Built-in pattern + dictionary recognizer (`elide-pattern`).
    /// At most one per analyzer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<PatternRecognizerParams>,
    /// External-backend NER recognizers (`elide-ner`). Each
    /// must have a unique `name`; runs in parallel with the
    /// rest.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ner: Vec<NerRecognizerParams>,
    /// External-backend LLM recognizers (`elide-llm`). Each
    /// must have a unique `name`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub llm: Vec<LlmRecognizerParams>,
}

/// Params for the `elide-pattern` recognizer.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(rename_all = "camelCase")]
pub struct PatternRecognizerParams {
    /// Load every pattern + dictionary shipped with
    /// `elide-pattern`. Implies the country-scoped
    /// jurisdictional pattern packs are active for the scope's
    /// jurisdictions.
    #[serde(default)]
    pub builtins: bool,
    /// Wrap the bare pattern recognizer in elide's
    /// `Enhanced<PatternRecognizer>` layer so per-label context
    /// keywords boost low-confidence matches before they leave
    /// the recognizer.
    #[serde(default)]
    pub context_enhanced: bool,
}

/// Params for one `elide-ner` recognizer instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NerRecognizerParams {
    /// Recognizer name — surfaces on the per-entity provenance
    /// trail so audits can tell which NER instance fired. Must
    /// be unique within the parent
    /// [`RecognizerParams::ner`](super::RecognizerParams::ner)
    /// list.
    pub name: String,
    /// Backend instantiation choice.
    pub backend: NerBackendParams,
}

/// How to instantiate the NER backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NerBackendParams {
    /// No-op backend; emits no entities. For tests, offline
    /// wiring, or skeleton runs.
    Mock,
    /// BentoML-hosted NER service. Engine wires the shared
    /// `elide-bento` client; per-request URL + model come from
    /// this variant.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
}

/// Params for one `elide-llm` recognizer instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LlmRecognizerParams {
    /// Recognizer name — surfaces on the per-entity provenance
    /// trail. Must be unique within the parent
    /// [`RecognizerParams::llm`](super::RecognizerParams::llm)
    /// list.
    pub name: String,
    /// Backend (provider) choice.
    pub backend: LlmBackendParams,
    /// Optional custom prompt template. `None` uses elide's
    /// default recognition prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

/// How to instantiate the LLM backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LlmBackendParams {
    /// No-op backend; emits no entities.
    Mock,
    /// OpenAI GPT provider.
    Openai {
        /// Model identifier (e.g. `"gpt-4o-mini"`).
        model: String,
    },
    /// Anthropic Claude provider.
    Anthropic {
        /// Model identifier (e.g. `"claude-3-5-sonnet-20241022"`).
        model: String,
    },
    /// Google Gemini provider.
    Google {
        /// Model identifier (e.g. `"gemini-1.5-flash"`).
        model: String,
    },
}

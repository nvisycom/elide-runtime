//! Per-recognizer specs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One recognizer to instantiate inside the request's analyzer.
///
/// Tagged enum keyed by recognizer kind; each variant carries the
/// recognizer-specific configuration the engine needs to build a
/// live instance from elide's builders.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecognizerSpec {
    /// Built-in pattern + dictionary recognizer (elide-pattern).
    Pattern(PatternRecognizerSpec),
    /// External-backend NER recognizer (elide-ner).
    Ner(NerRecognizerSpec),
    /// External-backend LLM recognizer (elide-llm).
    Llm(LlmRecognizerSpec),
}

/// Spec for the elide-pattern recognizer.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(rename_all = "camelCase")]
pub struct PatternRecognizerSpec {
    /// Load every pattern + dictionary shipped with elide-pattern.
    /// Implies the country-scoped jurisdictional pattern packs are
    /// active for the scope's jurisdictions.
    #[serde(default)]
    pub builtins: bool,
    /// Wrap the bare pattern recognizer in elide's
    /// `Enhanced<PatternRecognizer>` layer so per-label context
    /// keywords boost low-confidence matches before they leave the
    /// recognizer.
    #[serde(default)]
    pub context_enhanced: bool,
}

/// Spec for the elide-ner recognizer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NerRecognizerSpec {
    /// Recognizer name — surfaces on the per-entity provenance trail
    /// so audits can tell which NER instance fired.
    pub name: String,
    /// Backend instantiation choice.
    pub backend: NerBackendSpec,
    /// Labels this recognizer is allowed to emit. Empty means "no
    /// restriction" (the backend's full label set).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
}

/// How to instantiate the NER backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NerBackendSpec {
    /// No-op backend; emits no entities. For tests, offline wiring,
    /// or skeleton runs.
    Mock,
    /// BentoML-hosted NER service. Engine wires the shared
    /// `elide-bento` client; per-request URL + model come from this
    /// variant.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
}

/// Spec for the elide-llm recognizer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LlmRecognizerSpec {
    /// Recognizer name — surfaces on the per-entity provenance trail.
    pub name: String,
    /// Backend (provider) choice.
    pub backend: LlmBackendSpec,
    /// Optional custom prompt template. `None` uses elide's default
    /// recognition prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Labels this recognizer is allowed to emit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
}

/// How to instantiate the LLM backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LlmBackendSpec {
    /// No-op backend; emits no entities.
    Mock,
    /// OpenAI GPT provider. Model + per-request settings inline.
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

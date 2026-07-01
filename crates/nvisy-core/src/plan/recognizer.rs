//! Recognizer params: per-kind slots inside an
//! [`AnalyzerParams`].
//!
//! Three recognizer kinds — pattern, NER, LLM:
//!
//! - **Pattern** is at-most-one. Single regex/dictionary engine
//!   per analyzer; multi-pattern means accumulating into one
//!   instance's pattern list, not running two engines.
//! - **NER** is a list. Multiple NER backends (different model
//!   versions, language-specialised models, ensemble lanes) run
//!   in parallel; each is identified by [`name`] for
//!   provenance.
//! - **LLM** is a **deployment-owned lineup**. The wire only
//!   carries a boolean toggle: `true` runs every
//!   deployment-configured LLM recognizer whose modalities
//!   match the analyzer, `false` skips them. Provider, model,
//!   prompt, name, and credentials live in the deployment
//!   config (see `nvisy-engine`'s LLM config layer). Rationale:
//!   policies stay portable across deployments; the SaaS
//!   operator (or sidecar user) controls which model actually
//!   runs.
//!
//! [`AnalyzerParams`]: super::AnalyzerParams
//! [`name`]: NerRecognizerParams::name

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Recognizer slots an analyzer can fill. Pattern is
/// at-most-one, NER is a list, LLM is a deployment-owned
/// lineup gated by a boolean toggle.
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
    /// Run the deployment's LLM recognizer lineup. `false`
    /// skips LLM recognition entirely; `true` attaches every
    /// deployment-configured recognizer whose declared
    /// modalities match the analyzer's modality. When the
    /// deployment has no LLM recognizers configured, `true`
    /// fails the analyzer compile with a `Validation` error.
    #[serde(default, skip_serializing_if = "is_false")]
    pub llm: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Params for the `elide-pattern` recognizer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
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
///
/// The backend `kind` and its per-kind fields sit inline
/// alongside `name` — no nested `backend = { ... }` table. Serde
/// routes on `kind`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NerRecognizerParams {
    /// Recognizer name — surfaces on the per-entity provenance
    /// trail so audits can tell which NER instance fired. Must
    /// be unique within the parent
    /// [`RecognizerParams::ner`](super::RecognizerParams::ner)
    /// list.
    pub name: String,
    /// Backend selection + its per-kind fields, flattened onto
    /// the recognizer's wire shape.
    #[serde(flatten)]
    pub backend: NerBackendParams,
}

/// How to instantiate the NER backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum NerBackendParams {
    /// BentoML-hosted NER service. Engine wires the shared
    /// `elide-bento` client; per-request URL + model come from
    /// this variant.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
    /// No-op backend; emits no entities. Test-only — the wire
    /// only accepts `kind = "mock"` when the consuming crate
    /// enables the `test-utils` feature.
    #[cfg(feature = "test-utils")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
    Mock,
}


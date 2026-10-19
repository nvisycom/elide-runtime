//! LLM deployment configuration.
//!
//! A request's [`ComponentSelection`] names which recognizers to
//! run, by name or by tag; every detail of *how* one runs lives
//! here, on the deployment's side. Sidecar users configure their
//! own lineup; SaaS operators configure the lineup their tenants
//! share.
//!
//! ## Layout
//!
//! - [`LlmBackend`] is what one recognizer runs on: a source plus
//!   the modalities it attaches to. The modalities live here
//!   rather than on the shared [`Component`] because only an LLM
//!   reads them.
//! - [`LlmSource`] is the discriminated source enum, wrapping
//!   [`elide::recognition::llm::provider::Provider`]'s variants
//!   (with model + credentials) plus a test-only `Mock`.
//!
//! [`ComponentSelection`]: crate::component::ComponentSelection
//! - [`AttachTo`] tags one such modality.
//!
//! [`Component`]: crate::Component
//!
//! [`elide::recognition::llm::provider::Provider`]: https://docs.rs/elide-llm/latest/elide_llm/provider/enum.Provider.html

#[doc(inline)]
pub use elide::recognition::llm::provider::{AuthenticatedProvider, UnauthenticatedProvider};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::component::Backend;

/// Where a configured recognizer gets its LLM completions.
///
/// The four rig variants mirror
/// [`elide::recognition::llm::provider::Provider`] and forward its inner
/// payloads directly. The `Mock` variant exists only when the
/// consuming crate enables the `mocks` feature: the wire
/// rejects `kind = "mock"` in production builds.
#[derive(Debug, Clone)]
#[derive(Serialize, Deserialize, JsonSchema)]
// Denies unknown fields even though `Component` flattens this:
// the attribute is refused on the *outer* struct (the flattened
// fields reach it first and look unknown), but on the enum itself
// it catches a misspelled backend key -- `zero_retenton` for
// `zero_retention` -- which would otherwise be dropped, leaving a
// deployment believing it asked for something it did not.
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
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
    #[cfg(feature = "mocks")]
    #[cfg_attr(docsrs, doc(cfg(feature = "mocks")))]
    Mock,
}

/// The LLM backend a recognizer runs on: which provider, and which
/// analyzers it attaches to.
///
/// [`modalities`] lives here rather than on [`Component`] because
/// only an LLM reads it. Every other backend attaches where its own
/// modality dictates: an OCR enricher is an image enricher, and has
/// no choice to express.
///
/// [`modalities`]: Self::modalities
/// [`Component`]: crate::component::Component
#[derive(Debug, Clone)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LlmBackend {
    /// Source selection and its per-kind fields, flattened onto the
    /// recognizer's wire shape.
    #[serde(flatten)]
    pub source: LlmSource,
    /// Which analyzer modalities this recognizer attaches to.
    ///
    /// Empty is an error at compile time: a recognizer that
    /// attaches to no analyzer never runs.
    #[serde(default = "default_modalities")]
    pub modalities: Vec<AttachTo>,
}

/// Text, the modality an LLM recognizer attaches to when its
/// config does not say.
fn default_modalities() -> Vec<AttachTo> {
    vec![AttachTo::Text]
}

impl LlmSource {
    /// Provider slug for this source.
    fn provider(&self) -> &'static str {
        match self {
            Self::OpenAi(_) => "openai",
            Self::Anthropic(_) => "anthropic",
            Self::Gemini(_) => "gemini",
            Self::Ollama(_) => "ollama",
            #[cfg(feature = "mocks")]
            Self::Mock => "mock",
        }
    }
}

impl Backend for LlmBackend {
    fn provider(&self) -> &'static str {
        self.source.provider()
    }
}

/// Which analyzer modalities an LLM recognizer attaches to.
///
/// Text-only default because some models don't support vision;
/// opt in to `Image` explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttachTo {
    /// Attach to the text analyzer.
    Text,
    /// Attach to the image analyzer.
    Image,
}

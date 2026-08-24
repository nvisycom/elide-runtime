//! LLM deployment configuration.
//!
//! The wire's `RecognizerParams.llm` selects recognizers by
//! name (or the whole lineup); every detail about which LLM(s)
//! actually run lives here, on the deployment's side. Sidecar
//! users configure their own lineup; SaaS operators configure
//! the lineup their tenants share.
//!
//! ## Layout
//!
//! - [`LlmConfig`] is the top-level bag: the recognizer lineup.
//! - [`LlmBackend`] is what one recognizer runs on: a source plus
//!   the modalities it attaches to. The modalities live here
//!   rather than on the shared [`Component`] because only an LLM
//!   reads them.
//! - [`LlmSource`] is the discriminated source enum, wrapping
//!   [`elide::recognition::llm::provider::Provider`]'s variants
//!   (with model + credentials) plus a test-only `Mock`.
//! - [`AttachTo`] tags one such modality.
//!
//! [`Component`]: crate::Component
//!
//! [`elide::recognition::llm::provider::Provider`]: https://docs.rs/elide-llm/latest/elide_llm/provider/enum.Provider.html

#[doc(inline)]
pub use elide::recognition::llm::provider::{AuthenticatedProvider, UnauthenticatedProvider};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

/// The LLM backend a recognizer runs on: which provider, and which
/// analyzers it attaches to.
///
/// [`modalities`] lives here rather than on [`Component`] because
/// only an LLM reads it. Every other backend attaches where its own
/// modality dictates: an OCR enricher is an image enricher, and has
/// no choice to express.
///
/// [`modalities`]: Self::modalities
/// [`Component`]: crate::Component
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
            #[cfg(feature = "test-utils")]
            Self::Mock => "mock",
        }
    }
}

impl super::super::super::Backend for LlmBackend {
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

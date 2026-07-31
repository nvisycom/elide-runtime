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
//! - [`LlmRecognizer`] declares one recognizer instance: source
//!   + optional prompt + which modalities it attaches to.
//! - [`LlmSource`] is the discriminated source enum, wrapping
//!   [`elide::recognition::llm::provider::Provider`]'s variants
//!   (with model + credentials) plus a test-only `Mock`.
//! - [`LlmPrompt`] is the prompt spec: inline template, file
//!   path, or absent (uses elide's default recognition prompt).
//! - [`AttachTo`] tags which analyzer modalities a recognizer
//!   attaches to.
//!
//! [`elide::recognition::llm::provider::Provider`]: https://docs.rs/elide-llm/latest/elide_llm/provider/enum.Provider.html

mod attach;
mod prompt;
mod recognizer;

#[doc(inline)]
pub use elide::recognition::llm::provider::{AuthenticatedProvider, UnauthenticatedProvider};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::attach::AttachTo;
pub use self::prompt::LlmPrompt;
pub use self::recognizer::{LlmRecognizer, LlmSource};

/// Top-level LLM configuration. Loaded from the deployment's
/// `[llm]` config section.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    /// The recognizer lineup. Every entry runs on every analyzer
    /// whose modality it declares (see
    /// [`LlmRecognizer::modalities`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recognizers: Vec<LlmRecognizer>,
}

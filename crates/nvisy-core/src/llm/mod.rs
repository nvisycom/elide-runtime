//! LLM deployment configuration.
//!
//! The wire's `RecognizerParams.llm` is only a boolean — every
//! detail about which LLM(s) actually run lives here, on the
//! deployment's side. Sidecar users configure their own lineup;
//! SaaS operators configure the lineup their tenants share.
//!
//! ## Layout
//!
//! - [`LlmConfig`] is the top-level bag: the recognizer lineup.
//! - [`LlmRecognizer`] declares one recognizer instance:
//!   provider (with model + credentials, from
//!   [`elide_llm::provider::Provider`]) + optional prompt +
//!   which modalities it attaches to.
//! - [`LlmPrompt`] is the prompt spec — inline template, file
//!   path, or absent (uses elide's default recognition prompt).
//!
//! [`elide_llm::provider::Provider`]: https://docs.rs/elide-llm/latest/elide_llm/provider/enum.Provider.html

mod prompt;
mod provider;
mod recognizer;

pub use self::prompt::LlmPrompt;
pub use self::provider::LlmRecognizerModality;
pub use self::recognizer::{LlmBackendConfig, LlmRecognizer};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

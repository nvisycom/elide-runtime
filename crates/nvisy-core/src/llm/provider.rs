//! [`LlmRecognizerModality`]: which analyzer modality a
//! recognizer attaches to.
//!
//! The provider-selection wire shape lives on
//! [`elide_llm::provider::Provider`]; nvisy embeds it directly
//! on [`super::LlmRecognizer`] rather than mirroring it.
//!
//! [`elide_llm::provider::Provider`]: https://docs.rs/elide-llm/latest/elide_llm/provider/enum.Provider.html

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which analyzer modalities an LLM recognizer attaches to.
/// Text-only default because some models don't support vision;
/// opt in to `Image` explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LlmRecognizerModality {
    /// Attach to the text analyzer.
    Text,
    /// Attach to the image analyzer.
    Image,
}

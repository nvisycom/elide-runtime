//! [`AttachTo`]: which analyzer modality an LLM recognizer
//! attaches to.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

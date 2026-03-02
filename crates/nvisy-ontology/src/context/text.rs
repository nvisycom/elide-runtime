//! Text-modality reference data.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Textual reference values.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextData {
    /// One or more text values to match.
    pub values: Vec<String>,
}

//! Text reference data for direct comparison.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A labeled text value for reference matching.
///
/// The `key` is a human/LLM-readable label describing what this value
/// represents (e.g. `"full_name"`, `"account_number"`).  The `value` is
/// the literal string used for pattern matching.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextEntry {
    /// Human/LLM-readable label.
    pub key: String,
    /// Literal value for pattern matching.
    pub value: String,
}

/// Textual reference data as key-value pairs.
///
/// Keys describe *what* a value represents (for humans and LLMs);
/// values are the literal strings used for matching.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextData {
    /// Key-value pairs for matching.
    pub entries: Vec<TextEntry>,
}

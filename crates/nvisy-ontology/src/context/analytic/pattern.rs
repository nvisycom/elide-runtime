//! Pattern reference data for regex/glob matching.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A named pattern for detection matching.
///
/// `label` describes the intent (for humans/LLMs); `expression` is the
/// regex or glob used at detection time.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatternData {
    /// Human-readable label describing what this pattern matches.
    pub label: String,
    /// The regex or glob expression.
    pub expression: String,
    /// Whether this is a regex (`true`) or a glob/literal (`false`).
    #[serde(default)]
    pub is_regex: bool,
}

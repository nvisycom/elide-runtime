//! Pattern reference data for regex/glob matching.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A pattern expression with its type.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "syntax", rename_all = "snake_case")]
#[non_exhaustive]
pub enum PatternExpression {
    /// Regular expression pattern.
    Regex {
        /// The regex expression.
        expression: String,
    },
    /// Shell-style glob pattern.
    Glob {
        /// The glob expression.
        expression: String,
    },
}

/// A named pattern for detection matching.
///
/// `label` describes the intent (for humans/LLMs); `pattern` carries
/// the expression and its type.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatternData {
    /// Human-readable label describing what this pattern matches.
    pub label: String,
    /// The pattern expression and its type.
    #[serde(flatten)]
    pub pattern: PatternExpression,
}

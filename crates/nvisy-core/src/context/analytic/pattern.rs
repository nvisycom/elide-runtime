//! Pattern reference data for regex/glob matching.

use derive_more::From;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A regular expression pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegexPattern {
    /// The regex expression string.
    pub expression: String,
}

/// A shell-style glob pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GlobPattern {
    /// The glob expression string.
    pub expression: String,
}

/// A pattern expression with its syntax type.
#[derive(Debug, Clone, PartialEq, Eq, From, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "syntax", rename_all = "snake_case")]
#[non_exhaustive]
pub enum PatternExpression {
    /// Regular expression pattern.
    Regex(RegexPattern),
    /// Shell-style glob pattern.
    Glob(GlobPattern),
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

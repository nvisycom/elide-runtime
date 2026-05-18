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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::analytic::AnalyticVariant;

    #[test]
    fn pattern_expression_roundtrip_regex() {
        let pat = PatternExpression::Regex(RegexPattern {
            expression: r"^\d+$".to_owned(),
        });
        let json = serde_json::to_string(&pat).unwrap();
        assert!(json.contains(r#""syntax":"regex""#));
        assert!(json.contains(r#""expression":"^\\d+$""#));
        let back: PatternExpression = serde_json::from_str(&json).unwrap();
        assert_eq!(pat, back);
    }

    #[test]
    fn pattern_expression_roundtrip_glob() {
        let pat = PatternExpression::Glob(GlobPattern {
            expression: "*.txt".to_owned(),
        });
        let json = serde_json::to_string(&pat).unwrap();
        assert!(json.contains(r#""syntax":"glob""#));
        let back: PatternExpression = serde_json::from_str(&json).unwrap();
        assert_eq!(pat, back);
    }

    #[test]
    fn pattern_data_flatten_no_tag_collision() {
        // AnalyticVariant uses `tag = "kind"` and PatternExpression uses
        // `tag = "syntax"`. Flattening must keep both distinct.
        let entry = AnalyticVariant::Pattern(PatternData {
            label: "phone".to_owned(),
            pattern: PatternExpression::Regex(RegexPattern {
                expression: r"\d{3}-\d{4}".to_owned(),
            }),
        });
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains(r#""kind":"pattern""#));
        assert!(json.contains(r#""syntax":"regex""#));
        let back: AnalyticVariant = serde_json::from_str(&json).unwrap();
        match back {
            AnalyticVariant::Pattern(p) => {
                assert_eq!(p.label, "phone");
                assert!(matches!(p.pattern, PatternExpression::Regex(_)));
            }
            _ => panic!("expected Pattern variant"),
        }
    }
}

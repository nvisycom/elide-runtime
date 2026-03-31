//! [`ContextRule`]: co-occurrence context for confidence adjustment.

use serde::Deserialize;

/// Co-occurrence context rule for confidence adjustment.
///
/// When a pattern match is found, the surrounding text within `window`
/// characters is searched for any of the `keywords`. If at least one
/// keyword is found, the match confidence is increased by `boost`. If
/// no keywords are found, the confidence is decreased by `penalty`.
/// Both adjustments are clamped to `[0.0, 1.0]`.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "RawContextRule")]
pub struct ContextRule {
    /// Keywords to look for in surrounding text.
    pub keywords: Vec<String>,
    /// Number of characters before and after the match to search.
    pub window: usize,
    /// Confidence increase when at least one keyword is found.
    pub boost: f64,
    /// Confidence decrease when no keywords are found.
    pub penalty: f64,
    /// Whether keyword matching is case-sensitive.
    ///
    /// Defaults to `false`: case-insensitive.
    pub case_sensitive: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct RawContextRule {
    keywords: Vec<String>,
    #[serde(default = "default_window")]
    window: usize,
    #[serde(default = "default_boost")]
    boost: f64,
    #[serde(default)]
    penalty: f64,
    #[serde(default)]
    case_sensitive: bool,
}

impl TryFrom<RawContextRule> for ContextRule {
    type Error = String;

    fn try_from(raw: RawContextRule) -> Result<Self, Self::Error> {
        if !(0.0..=1.0).contains(&raw.boost) {
            return Err(format!(
                "context rule boost must be in [0.0, 1.0], got {}",
                raw.boost
            ));
        }
        if !(0.0..=1.0).contains(&raw.penalty) {
            return Err(format!(
                "context rule penalty must be in [0.0, 1.0], got {}",
                raw.penalty
            ));
        }
        Ok(Self {
            keywords: raw.keywords,
            window: raw.window,
            boost: raw.boost,
            penalty: raw.penalty,
            case_sensitive: raw.case_sensitive,
        })
    }
}

fn default_window() -> usize {
    200
}

fn default_boost() -> f64 {
    0.1
}

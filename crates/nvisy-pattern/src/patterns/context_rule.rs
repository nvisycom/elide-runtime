//! [`ContextRule`] — co-occurrence context for span-level confidence boosting.

use serde::Deserialize;

/// Co-occurrence context rule for span-level confidence boosting.
///
/// When a pattern match is found, nearby spans are searched for any of the
/// `keywords`.  If at least one keyword is present within `window` spans,
/// the match confidence is increased by `boost` (clamped to `[0.0, 1.0]`).
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "RawContextRule")]
pub struct ContextRule {
    /// Keywords to look for in nearby spans.
    pub keywords: Vec<String>,
    /// Number of spans before and after the match span to search.
    pub window: usize,
    /// Confidence adjustment when at least one keyword is found.
    /// Must be in the range `[0.0, 1.0]`.
    pub boost: f64,
    /// Whether keyword matching is case-sensitive.
    ///
    /// Defaults to `false` (case-insensitive).
    pub case_sensitive: bool,
}

/// Serde intermediary that mirrors the JSON shape before validation.
#[derive(Deserialize)]
struct RawContextRule {
    keywords: Vec<String>,
    #[serde(default = "default_window")]
    window: usize,
    #[serde(default = "default_boost")]
    boost: f64,
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
        Ok(Self {
            keywords: raw.keywords,
            window: raw.window,
            boost: raw.boost,
            case_sensitive: raw.case_sensitive,
        })
    }
}

fn default_window() -> usize {
    3
}

fn default_boost() -> f64 {
    0.1
}

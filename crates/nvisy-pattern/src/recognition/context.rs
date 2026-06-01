//! [`Context`]: per-pattern keyword-boost declaration.
//!
//! A [`Pattern`] or [`Dictionary`] that wants its matches boosted
//! when keywords appear nearby attaches a `Context` describing the
//! keyword list and optionally overriding the enhancer's window /
//! boost defaults.
//!
//! The boost itself is applied by a separate
//! [`ContextEnhancer`](crate::ContextEnhancer) post-pass that walks
//! recognizer output, looks the pattern up by name, reads its
//! [`Context`], and applies the boost when any keyword appears in
//! the surrounding window. Recognizers themselves do not consult
//! [`Context`] — they emit at the base score.
//!
//! [`Pattern`]: crate::Pattern
//! [`Dictionary`]: crate::Dictionary

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Per-pattern context-boost declaration.
///
/// `window` and `boost` are `Option<_>` because the common case is
/// "I have keywords, use the enhancer's defaults." Set them only
/// when the pattern needs a different policy than the enhancer's
/// global defaults.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// Keywords whose presence near a match boosts the confidence
    /// score. Empty means no enhancement is performed for this
    /// pattern.
    pub keywords: Vec<String>,
    /// Override of the enhancer's default window (in bytes on each
    /// side of the match). `None` falls back to the enhancer's
    /// default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<usize>,
    /// Override of the enhancer's default boost (additive bump to
    /// the match's confidence). `None` falls back to the enhancer's
    /// default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boost: Option<f64>,
}

impl Context {
    /// Construct with a keyword list. Window and boost default to
    /// `None` (fall back to the enhancer's defaults at boost time).
    #[must_use]
    pub fn new(keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            keywords: keywords.into_iter().map(Into::into).collect(),
            window: None,
            boost: None,
        }
    }

    /// Override the enhancer's window setting for this pattern.
    #[must_use]
    pub fn with_window(mut self, window: usize) -> Self {
        self.window = Some(window);
        self
    }

    /// Override the enhancer's boost setting for this pattern.
    #[must_use]
    pub fn with_boost(mut self, boost: f64) -> Self {
        self.boost = Some(boost);
        self
    }
}

impl<S: Into<String>> From<Vec<S>> for Context {
    fn from(keywords: Vec<S>) -> Self {
        Self::new(keywords)
    }
}

impl<const N: usize> From<[&str; N]> for Context {
    fn from(keywords: [&str; N]) -> Self {
        Self::new(keywords)
    }
}

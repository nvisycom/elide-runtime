//! [`Context`]: per-source keyword-boost declaration.
//!
//! Carried by anything that declares context — per-rule for
//! patterns (each `Regex`/`Dictionary` may declare one),
//! per-recognizer for NER (a single `default_context` on
//! `NerRecognizer`). The shape is identical regardless of who
//! registers it; the difference is only *what name* gets stored
//! against it in the [`ContextRegistry`].
//!
//! [`ContextRegistry`]: super::ContextRegistry
//!
//! `window` and `boost` are `Option<_>` so the common case is "I
//! have keywords; use the enhancer's defaults." Override only when
//! the source needs a different policy than the enhancer's global
//! settings.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Per-source context-boost declaration.
///
/// Anything that wants to participate in post-recognition keyword
/// boosting registers one of these against its name in a
/// [`ContextRegistry`].
///
/// [`ContextRegistry`]: super::ContextRegistry
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// Keywords whose presence near a match boosts the entity's
    /// confidence. Empty list means "registered, but no boost
    /// possible" — the enhancer skips this source.
    pub keywords: Vec<String>,
    /// Override of the enhancer's default window (in bytes on each
    /// side of the match). `None` defers to the enhancer's
    /// configured default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<usize>,
    /// Override of the enhancer's default additive boost. `None`
    /// defers to the enhancer's configured default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boost: Option<f64>,
}

impl Context {
    /// Construct with a keyword list. Window and boost default to
    /// `None` (use the enhancer's defaults).
    #[must_use]
    pub fn new(keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            keywords: keywords.into_iter().map(Into::into).collect(),
            window: None,
            boost: None,
        }
    }

    /// Override the enhancer's window setting for this source.
    #[must_use]
    pub fn with_window(mut self, window: usize) -> Self {
        self.window = Some(window);
        self
    }

    /// Override the enhancer's boost setting for this source.
    #[must_use]
    pub fn with_boost(mut self, boost: f64) -> Self {
        self.boost = Some(boost);
        self
    }

    /// Whether this context carries no boost-eligible keywords.
    /// Empty contexts are skipped by the enhancer.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keywords.is_empty()
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

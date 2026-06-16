//! [`SuppressionParams`]: caller-supplied allow lists consumed by
//! [`SuppressionLayer`].
//!
//! Three independent allow-list shapes apply by union — an entity
//! is dropped when **any** of them fires:
//!
//! - exact ASCII case-insensitive equality
//! - substring containment
//! - regex match
//!
//! All three operate on the entity's resolved text (sliced from
//! the source via [`TextAt::text_at`]), not the surrounding
//! document.
//!
//! [`SuppressionLayer`]: super::SuppressionLayer
//! [`TextAt::text_at`]: nvisy_core::extraction::TextAt::text_at

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Caller-supplied allow lists consumed by [`SuppressionLayer`].
///
/// All three lists default to empty; the layer short-circuits as
/// a fast no-op when every list is empty.
///
/// [`SuppressionLayer`]: super::SuppressionLayer
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SuppressionParams {
    /// Drop entities whose matched text equals one of these values
    /// (ASCII case-insensitive). Use for known false-positive
    /// values like `noreply@yourcompany.com`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_values: Vec<String>,
    /// Drop entities whose matched text contains one of these
    /// values as a substring (ASCII case-insensitive). Use when an
    /// over-matching recognizer surrounds a known false-positive
    /// value with extra text.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_values_substring: Vec<String>,
    /// Drop entities whose matched text matches one of these
    /// regular expressions. Compiled once at layer construction.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_values_regex: Vec<String>,
}

impl SuppressionParams {
    /// Empty params: every allow list defaults to empty.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the exact-match list.
    #[must_use]
    pub fn with_allow_values(mut self, values: Vec<String>) -> Self {
        self.allow_values = values;
        self
    }

    /// Set the substring-match list.
    #[must_use]
    pub fn with_allow_values_substring(mut self, values: Vec<String>) -> Self {
        self.allow_values_substring = values;
        self
    }

    /// Set the regex-match list.
    #[must_use]
    pub fn with_allow_values_regex(mut self, values: Vec<String>) -> Self {
        self.allow_values_regex = values;
        self
    }

    /// Return `true` when no allow-list values are configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.allow_values.is_empty()
            && self.allow_values_substring.is_empty()
            && self.allow_values_regex.is_empty()
    }
}

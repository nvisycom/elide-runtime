//! [`Recognition`]: the vocabulary one request introduces, and
//! how to detect it.
//!
//! elide ships a label set, and most requests want only that. A
//! caller with something of their own — an employee id, a client
//! codename, an internal case reference — supplies it here: the
//! labels, and how to find them.
//!
//! Per request rather than per policy. Detection vocabulary is
//! request-wide already: the engine unions every policy's labels
//! into one catalog before a recognizer runs, so scoping a matcher
//! to the policy that declared it was an artificial constraint —
//! two policies covering the same custom label had to declare it
//! twice. A policy decides what *happens* to sensitive data; this
//! decides what the engine can *find*.

mod matcher;

use elide_core::entity::Label;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::matcher::{CustomMatcher, MatchOn};

/// Labels a request introduces, and how to detect them.
///
/// Both halves are needed. A label on its own joins the
/// vocabulary and is never found, because nothing looks for it;
/// a matcher on its own names a label the catalog does not hold.
/// Declaring one without the other is refused rather than
/// silently ignored.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Recognition {
    /// Label schemas elide does not ship.
    ///
    /// Only for labels outside the shipped set: one whose id
    /// collides with a builtin is refused, since it would shadow
    /// elide's own definition.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<Label>,
    /// How to detect the labels [`custom`] introduces.
    ///
    /// Each names a label this request declares. Compiled fresh
    /// per request, so a request introducing none costs nothing.
    ///
    /// [`custom`]: Self::custom
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matchers: Vec<CustomMatcher>,
}

impl Recognition {
    /// A request introducing nothing: elide's shipped labels only.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this request introduces no vocabulary of its own,
    /// the common case.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.custom.is_empty() && self.matchers.is_empty()
    }
}

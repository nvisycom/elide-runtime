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
/// The two halves are independent, and only one pairing is an
/// error.
///
/// A label with no matcher is legitimate: it joins the catalog, a
/// policy may scope it, and a reviewer may add entities under it
/// through [`Edit::Add`]. Nothing detects it automatically — there
/// is no matcher to look for it — which is the point when the
/// label marks something only a human can recognise.
///
/// A matcher with no label is refused, since it would detect into
/// a vocabulary the catalog never held.
///
/// [`Edit::Add`]: https://docs.rs/elide-review
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

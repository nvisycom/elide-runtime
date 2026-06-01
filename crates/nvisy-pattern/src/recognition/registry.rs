//! [`PatternRegistry`]: a curated bundle of [`Regex`]es and
//! [`Dictionary`]s that downstream consumers borrow.
//!
//! Both
//! [`PatternRecognizer`](super::PatternRecognizer) and
//! [`ContextEnhancer`](crate::enhancement::ContextEnhancer) accept a
//! `PatternRegistry`, pulling the data they need to do their job
//! (compiled scanners on one side, keyword lookup on the other).
//! Centralising the rule set here means no duplication of
//! [`Regex`] / [`Dictionary`] storage between the two consumers.

use super::dictionary::Dictionary;
use super::regex_rule::Regex;

/// Bundle of regexes and dictionaries shared by every downstream
/// consumer.
///
/// Cheap to clone (`Vec` of small structs); typically built once
/// per process from the [`shipped`](crate::shipped) helpers plus
/// any caller-supplied custom rules.
#[derive(Debug, Clone, Default)]
pub struct PatternRegistry {
    regexes: Vec<Regex>,
    dictionaries: Vec<Dictionary>,
}

impl PatternRegistry {
    /// Construct an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register one regex. Call once per regex; the registry
    /// accumulates them in registration order.
    #[must_use]
    pub fn with_pattern(mut self, regex: Regex) -> Self {
        self.regexes.push(regex);
        self
    }

    /// Register one dictionary. Call once per dictionary; the
    /// registry accumulates them in registration order.
    #[must_use]
    pub fn with_dictionary(mut self, dictionary: Dictionary) -> Self {
        self.dictionaries.push(dictionary);
        self
    }

    /// Borrow the registered regexes.
    #[must_use]
    pub fn patterns(&self) -> &[Regex] {
        &self.regexes
    }

    /// Borrow the registered dictionaries.
    #[must_use]
    pub fn dictionaries(&self) -> &[Dictionary] {
        &self.dictionaries
    }
}

impl FromIterator<Regex> for PatternRegistry {
    fn from_iter<I: IntoIterator<Item = Regex>>(iter: I) -> Self {
        Self {
            regexes: iter.into_iter().collect(),
            dictionaries: Vec::new(),
        }
    }
}

impl FromIterator<Dictionary> for PatternRegistry {
    fn from_iter<I: IntoIterator<Item = Dictionary>>(iter: I) -> Self {
        Self {
            regexes: Vec::new(),
            dictionaries: iter.into_iter().collect(),
        }
    }
}

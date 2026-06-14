//! [`PatternRegistry`]: a curated bundle of [`Regex`]es and
//! [`Dictionary`]s that downstream consumers borrow.
//!
//! Both [`PatternRecognizer`] and the shared [`ContextEnhancer`]
//! consume a registry — the recognizer compiles its rules into
//! pooled scanners; the enhancer reads per-rule context keywords
//! via [`PatternRegistry::context_registry`].
//!
//! Centralising the rule set here means no duplication of
//! [`Regex`] / [`Dictionary`] storage between the two consumers.
//!
//! [`PatternRecognizer`]: super::PatternRecognizer
//! [`ContextEnhancer`]: nvisy_core::context::ContextEnhancer

use nvisy_core::context::ContextRegistry;
use nvisy_core::entity::EntityLabelCatalog;

use super::dictionary::Dictionary;
use super::regex_rule::Regex;
use crate::shipped;

/// Bundle of regexes and dictionaries shared by every downstream
/// consumer.
///
/// Cheap to clone (`Vec` of small structs). Construct via
/// [`PatternRegistry::new`] for an empty registry,
/// [`PatternRegistry::builtin`] for the shipped registry (every
/// built-in regex + dictionary), or chain [`with_pattern`] /
/// [`with_dictionary`] / [`with_builtin_patterns`] /
/// [`with_builtin_dictionaries`] to mix custom rules in.
///
/// [`with_pattern`]: PatternRegistry::with_pattern
/// [`with_dictionary`]: PatternRegistry::with_dictionary
/// [`with_builtin_patterns`]: PatternRegistry::with_builtin_patterns
/// [`with_builtin_dictionaries`]: PatternRegistry::with_builtin_dictionaries
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

    /// Construct the shipped registry: every built-in regex pattern
    /// and every built-in dictionary, in registration order.
    /// Shorthand for `PatternRegistry::new().with_builtin_patterns().with_builtin_dictionaries()`.
    #[must_use]
    pub fn builtin() -> Self {
        Self::new()
            .with_builtin_patterns()
            .with_builtin_dictionaries()
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

    /// Register every shipped built-in regex pattern in registration
    /// order. Replaces the common `for p in patterns::all() { reg =
    /// reg.with_pattern(p); }` boilerplate.
    #[must_use]
    pub fn with_builtin_patterns(mut self) -> Self {
        self.regexes.extend(shipped::patterns::all());
        self
    }

    /// Register every shipped built-in dictionary in registration
    /// order. Replaces the common `dictionaries::all().into_iter()
    /// .fold(reg, PatternRegistry::with_dictionary)` boilerplate.
    #[must_use]
    pub fn with_builtin_dictionaries(mut self) -> Self {
        self.dictionaries.extend(shipped::dictionaries::all());
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

    /// Drop every regex and dictionary whose `label` is not
    /// registered in `catalog`. Used to build a per-request
    /// registry from the workspace template — patterns that would
    /// emit labels no policy declared never run.
    #[must_use]
    pub fn filter_by_catalog(mut self, catalog: &EntityLabelCatalog) -> Self {
        self.regexes
            .retain(|r| catalog.lookup(r.label.as_str()).is_some());
        self.dictionaries
            .retain(|d| catalog.lookup(d.label.as_str()).is_some());
        self
    }

    /// `true` when the registry has no regexes and no dictionaries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.regexes.is_empty() && self.dictionaries.is_empty()
    }

    /// Build a [`ContextRegistry`] containing every per-rule
    /// context keyword declaration in this registry.
    ///
    /// Each [`Regex`] and [`Dictionary`] that declares a non-empty
    /// context contributes one entry, keyed on its rule name.
    /// Rules without context declarations are skipped.
    ///
    /// Use this to wire the
    /// [`ContextEnhancer`]
    /// against the same source of truth the recognizer compiles
    /// from — no duplication of keyword data between rule
    /// registration and enhancer construction.
    ///
    /// [`ContextEnhancer`]: nvisy_core::context::ContextEnhancer
    #[must_use]
    pub fn context_registry(&self) -> ContextRegistry {
        let mut registry = ContextRegistry::new();
        for r in &self.regexes {
            registry = registry.with_entry(r.name.clone(), r.context.clone());
        }
        for d in &self.dictionaries {
            registry = registry.with_entry(d.name.clone(), d.context.clone());
        }
        registry
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

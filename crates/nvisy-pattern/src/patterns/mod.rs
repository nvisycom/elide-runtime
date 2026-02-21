//! Built-in detection patterns.
//!
//! Each pattern is a JSON file under `assets/patterns/` that describes how
//! to detect a single entity type.  Files are embedded at compile time with
//! `include_dir!` and auto-discovered by [`PatternRegistry::load_builtins`].
//!
//! # Key types
//!
//! - [`Pattern`]: trait implemented by every pattern.
//! - [`JsonPattern`]: concrete implementation deserialized from JSON.
//! - [`MatchSource`]: whether matching is regex-based or dictionary-based.
//! - [`PatternRegistry`]: sorted collection with O(log n) lookup by name.
//! - [`JsonPatternError`] / [`JsonPatternWarning`]: load-time diagnostics.
//!
//! [`Pattern`]: crate::patterns::Pattern
//! [`JsonPattern`]: crate::patterns::JsonPattern
//! [`MatchSource`]: crate::patterns::MatchSource
//! [`PatternRegistry`]: crate::patterns::PatternRegistry
//! [`PatternRegistry::load_builtins`]: crate::patterns::PatternRegistry::load_builtins
//! [`JsonPatternError`]: crate::patterns::JsonPatternError
//! [`JsonPatternWarning`]: crate::patterns::JsonPatternWarning

mod json_pattern;
mod pattern;

pub use json_pattern::{JsonPattern, JsonPatternWarning};
pub use pattern::{BoxPattern, MatchSource, Pattern};

use std::collections::BTreeMap;
use std::sync::LazyLock;

use include_dir::{Dir, include_dir};

/// A registry of named [`Pattern`] definitions with O(log n) lookup.
///
/// Use [`load_builtins`] to create a registry pre-populated with
/// the compile-time-embedded pattern files.
///
/// [`load_builtins`]: Self::load_builtins
pub struct PatternRegistry {
    inner: BTreeMap<String, BoxPattern>,
}

impl std::fmt::Debug for PatternRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.inner.keys().map(|s| s.as_str()).collect();
        f.debug_struct("PatternRegistry")
            .field("len", &self.inner.len())
            .field("names", &names)
            .finish()
    }
}

impl PatternRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    /// Insert a pattern, keyed by its [`Pattern::name`].
    pub fn insert(&mut self, pattern: BoxPattern) {
        let name = pattern.name().to_owned();
        self.inner.insert(name, pattern);
    }

    /// Look up a pattern by name.
    pub fn get(&self, name: &str) -> Option<&dyn Pattern> {
        self.inner.get(name).map(|b| b.as_ref())
    }

    /// All patterns in deterministic (alphabetical) order.
    pub fn values(&self) -> Vec<&dyn Pattern> {
        self.inner.values().map(|b| b.as_ref()).collect()
    }

    /// Total number of registered patterns.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Load all `.json` files from the embedded `assets/patterns/`
    /// directory and return a populated registry.
    ///
    /// Files that fail to parse are logged as warnings and skipped.
    #[tracing::instrument(name = "patterns.load_builtins", fields(count))]
    pub fn load_builtins() -> Self {
        static PATTERN_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/patterns");

        let mut reg = Self::new();

        for file in PATTERN_DIR.files() {
            let path = file.path();

            let Some("json") = path.extension().and_then(|e| e.to_str()) else {
                tracing::warn!(
                    path = %path.display(),
                    "skipping non-JSON file in patterns directory",
                );
                continue;
            };

            let (pattern, warnings) = match JsonPattern::from_bytes(file.contents()) {
                Ok(pair) => pair,
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "failed to load pattern, skipping",
                    );
                    continue;
                }
            };

            for w in &warnings {
                match w {
                    JsonPatternWarning::UnknownCategory { pattern, slug } => {
                        tracing::warn!(%pattern, category = %slug, "unrecognised category falls through to Custom");
                    }
                    JsonPatternWarning::UnknownValidator { pattern, validator } => {
                        tracing::warn!(%pattern, %validator, "unknown validator name, pattern will have no post-match validation");
                    }
                }
            }

            tracing::trace!(
                name = %pattern.name(),
                category = %pattern.category(),
                entity_kind = %pattern.entity_kind(),
                match_source = ?pattern.match_source(),
                validator = ?pattern.validator_name(),
                "pattern loaded",
            );
            reg.insert(Box::new(pattern));
        }

        tracing::Span::current().record("count", reg.len());
        tracing::debug!("built-in patterns loaded");
        reg
    }
}

impl Default for PatternRegistry {
    fn default() -> Self {
        Self::new()
    }
}

static BUILTIN_REGISTRY: LazyLock<PatternRegistry> =
    LazyLock::new(PatternRegistry::load_builtins);

/// Return a reference to the lazily-initialised built-in [`PatternRegistry`].
pub fn builtin_registry() -> &'static PatternRegistry {
    &BUILTIN_REGISTRY
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validators::ValidatorResolver;
    use nvisy_core::data::{EntityCategory, EntityKind};

    fn registry() -> &'static PatternRegistry {
        builtin_registry()
    }

    #[test]
    fn loads_all_patterns() {
        let patterns = registry().values();
        assert!(!patterns.is_empty());
        assert!(patterns.len() >= 10);
    }

    #[test]
    fn pattern_names_are_sorted() {
        let names: Vec<&str> = registry().values().iter().map(|p| p.name()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn get_known_pattern() {
        let ssn = registry().get("ssn").unwrap();
        assert_eq!(ssn.name(), "ssn");
        assert_eq!(*ssn.category(), EntityCategory::Pii);
        assert_eq!(ssn.entity_kind(), EntityKind::GovernmentId);
        assert!(ssn.validator_name().is_some());
        assert!(ssn.confidence() > 0.0);
    }

    #[test]
    fn all_patterns_have_nonempty_fields() {
        for p in registry().values() {
            assert!(!p.name().is_empty(), "pattern name is empty");
            match p.match_source() {
                MatchSource::Regex(re) => assert!(!re.is_empty(), "regex is empty for {}", p.name()),
                MatchSource::Dictionary(d) => assert!(!d.is_empty(), "dictionary is empty for {}", p.name()),
            }
            assert!(p.confidence() > 0.0, "confidence is 0 for {}", p.name());
            assert!(p.confidence() <= 1.0, "confidence > 1 for {}", p.name());
        }
    }

    #[test]
    fn all_regex_patterns_compile() {
        for p in registry().values() {
            if let MatchSource::Regex(re) = p.match_source() {
                assert!(
                    regex::Regex::new(re).is_ok(),
                    "pattern {} failed to compile: {}",
                    p.name(),
                    re,
                );
            }
        }
    }

    #[test]
    fn ssn_pattern_has_validator() {
        let resolver = ValidatorResolver::builtins();
        let ssn = registry().get("ssn").unwrap();
        assert_eq!(ssn.validator_name(), Some("ssn"));
        let validate = resolver.resolve("ssn").unwrap();
        assert!(validate("123-45-6789"));
        assert!(!validate("000-00-0000"));
    }

    #[test]
    fn credit_card_pattern_has_luhn_validator() {
        let resolver = ValidatorResolver::builtins();
        let cc = registry().get("credit-card").unwrap();
        assert_eq!(*cc.category(), EntityCategory::Financial);
        assert_eq!(cc.validator_name(), Some("luhn"));
        let validate = resolver.resolve("luhn").unwrap();
        assert!(validate("4539 1488 0343 6467"));
    }

    #[test]
    fn pattern_categories_are_correct() {
        let reg = registry();
        assert_eq!(*reg.get("email").unwrap().category(), EntityCategory::Pii);
        assert_eq!(*reg.get("aws-key").unwrap().category(), EntityCategory::Credentials);
        assert_eq!(*reg.get("credit-card").unwrap().category(), EntityCategory::Financial);
    }

    #[test]
    fn entity_kinds_match_expected() {
        let reg = registry();
        assert_eq!(reg.get("ssn").unwrap().entity_kind(), EntityKind::GovernmentId);
        assert_eq!(reg.get("email").unwrap().entity_kind(), EntityKind::EmailAddress);
        assert_eq!(reg.get("phone").unwrap().entity_kind(), EntityKind::PhoneNumber);
        assert_eq!(reg.get("credit-card").unwrap().entity_kind(), EntityKind::PaymentCard);
        assert_eq!(reg.get("aws-key").unwrap().entity_kind(), EntityKind::ApiKey);
        assert_eq!(reg.get("github-token").unwrap().entity_kind(), EntityKind::AuthToken);
        assert_eq!(reg.get("ipv4").unwrap().entity_kind(), EntityKind::IpAddress);
    }

    #[test]
    fn no_duplicate_pattern_names() {
        let all = registry().values();
        let names: Vec<_> = all.iter().map(|p| p.name()).collect();
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(names.len(), unique.len(), "duplicate pattern names found");
    }

    #[test]
    fn load_builtins_auto_discovers() {
        let reg = PatternRegistry::load_builtins();
        assert_eq!(reg.len(), 27);
    }

    #[test]
    fn registry_insert_and_get() {
        let json = br#"{
            "name": "test",
            "category": "pii",
            "entity_type": "government_id",
            "pattern": "\\d+",
            "confidence": 0.9
        }"#;
        let (pattern, _warnings) = JsonPattern::from_bytes(json).unwrap();

        let mut reg = PatternRegistry::new();
        reg.insert(Box::new(pattern));

        assert_eq!(reg.len(), 1);
        assert_eq!(reg.get("test").unwrap().name(), "test");
    }
}

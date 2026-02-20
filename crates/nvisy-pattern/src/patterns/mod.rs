//! Built-in regex pattern definitions and validation helpers.
//!
//! Each pattern lives in its own JSON file under `assets/patterns/`
//! and is embedded at compile time.  The [`PatternRegistry`] auto-discovers
//! all `.json` files in the directory.

mod json_pattern;
mod pattern;
/// Checksum and format validators used by pattern definitions.
pub mod validators;

pub use json_pattern::JsonPattern;
pub use pattern::{BoxPattern, Pattern};

use std::sync::LazyLock;

use include_dir::{Dir, include_dir};
use crate::registry::Registry;

/// Deserialize a JSON blob into a [`JsonPattern`].
///
/// Returns `None` and warns if the JSON contains an unrecognised `entity_type`.
fn parse_pattern(bytes: &[u8]) -> Option<JsonPattern> {
    match serde_json::from_slice::<JsonPattern>(bytes) {
        Ok(p) => {
            p.warn_on_load();
            Some(p)
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to parse pattern file, skipping");
            None
        }
    }
}

/// A registry of named pattern definitions with O(log n) lookup.
pub struct PatternRegistry {
    inner: Registry<BoxPattern>,
}

impl std::fmt::Debug for PatternRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PatternRegistry")
            .field("len", &self.inner.len())
            .field("names", &self.inner.names())
            .finish()
    }
}

impl PatternRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            inner: Registry::new(),
        }
    }

    /// Insert a pattern into the registry.
    pub fn insert(&mut self, name: String, pattern: BoxPattern) {
        self.inner.insert(name, pattern);
    }

    /// Look up a pattern by name.
    pub fn get(&self, name: &str) -> Option<&dyn Pattern> {
        self.inner.get(name).map(|b| b.as_ref())
    }

    /// All patterns in deterministic (alphabetical) order.
    pub fn values(&self) -> Vec<&dyn Pattern> {
        self.inner.values().into_iter().map(|b| b.as_ref()).collect()
    }

    /// All pattern names in deterministic (alphabetical) order.
    pub fn names(&self) -> Vec<&str> {
        self.inner.names()
    }

    /// Total number of registered patterns.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Load all `.json` files from the embedded `assets/patterns/`
    /// directory and return a populated registry.
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

            let Some(pattern) = parse_pattern(file.contents()) else {
                continue;
            };

            tracing::trace!(
                name = %pattern.name(),
                category = %pattern.category(),
                entity_kind = %pattern.entity_kind(),
                has_validator = pattern.has_validator(),
                "pattern loaded",
            );
            let name = pattern.name().to_owned();
            reg.insert(name, Box::new(pattern));
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

/// Get a reference to the built-in [`PatternRegistry`].
pub fn builtin_registry() -> &'static PatternRegistry {
    &BUILTIN_REGISTRY
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let names = registry().names();
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
        assert!(ssn.has_validator());
        assert!(ssn.confidence() > 0.0);
    }

    #[test]
    fn get_unknown_pattern_returns_none() {
        assert!(registry().get("nonexistent").is_none());
    }

    #[test]
    fn all_patterns_have_nonempty_fields() {
        for p in registry().values() {
            assert!(!p.name().is_empty(), "pattern name is empty");
            assert!(!p.pattern_str().is_empty(), "pattern_str is empty for {}", p.name());
            assert!(p.confidence() > 0.0, "confidence is 0 for {}", p.name());
            assert!(p.confidence() <= 1.0, "confidence > 1 for {}", p.name());
        }
    }

    #[test]
    fn all_patterns_compile_as_regex() {
        for p in registry().values() {
            assert!(
                regex::Regex::new(p.pattern_str()).is_ok(),
                "pattern {} failed to compile: {}",
                p.name(),
                p.pattern_str(),
            );
        }
    }

    #[test]
    fn ssn_pattern_has_validator() {
        let ssn = registry().get("ssn").unwrap();
        assert!(ssn.has_validator());
        assert!(ssn.validate("123-45-6789"));
        assert!(!ssn.validate("000-00-0000"));
    }

    #[test]
    fn credit_card_pattern_has_luhn_validator() {
        let cc = registry().get("credit-card").unwrap();
        assert_eq!(*cc.category(), EntityCategory::Financial);
        assert!(cc.has_validator());
        assert!(cc.validate("4539 1488 0343 6467"));
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
    fn pattern_definition_is_debug() {
        let reg = registry();
        let _ = format!("{:?}", reg);
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
        assert_eq!(reg.len(), 10);
    }

    #[test]
    fn registry_insert_and_get() {
        let json = r#"{
            "name": "test",
            "category": "pii",
            "entity_type": "government_id",
            "pattern": "\\d+",
            "confidence": 0.9
        }"#;
        let pattern: JsonPattern = serde_json::from_str(json).unwrap();

        let mut reg = PatternRegistry::new();
        reg.insert("test".into(), Box::new(pattern));

        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());
        assert_eq!(reg.get("test").unwrap().name(), "test");
    }
}

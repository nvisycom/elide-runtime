//! Built-in regex pattern definitions and validation helpers.
//!
//! Each pattern lives in its own JSON file under `assets/patterns/`
//! and is embedded at compile time.  The [`PatternRegistry`] auto-discovers
//! all `.json` files in the directory.

mod definition;
/// Checksum and format validators used by pattern definitions.
pub mod validators;

pub use definition::PatternDefinition;

use std::collections::BTreeMap;
use std::sync::LazyLock;

use include_dir::{Dir, include_dir};
use nvisy_core::data::EntityCategory;

/// JSON representation of a pattern loaded from disk.
#[derive(Debug, Clone, serde::Deserialize)]
struct PatternJson {
    name: String,
    category: String,
    entity_type: String,
    pattern: String,
    confidence: f64,
    #[serde(default)]
    validator: Option<String>,
}

/// Resolves a validator name string to its corresponding validation function.
fn resolve_validator(name: &str) -> Option<fn(&str) -> bool> {
    match name {
        "ssn" => Some(validators::validate_ssn),
        "luhn" => Some(validators::luhn_check),
        _ => None,
    }
}

/// Parse a single JSON blob into a [`PatternDefinition`].
fn parse_pattern(bytes: &[u8]) -> PatternDefinition {
    let p: PatternJson = serde_json::from_slice(bytes).expect("failed to parse pattern file");

    if let Some(ref v) = p.validator {
        if resolve_validator(v).is_none() {
            tracing::warn!(
                pattern = %p.name,
                validator = %v,
                "unknown validator name, pattern will have no post-match validation",
            );
        }
    }

    PatternDefinition {
        category: EntityCategory::from_slug(&p.category),
        validate: p.validator.as_deref().and_then(resolve_validator),
        name: p.name,
        entity_type: p.entity_type,
        pattern_str: p.pattern,
        confidence: p.confidence,
    }
}

/// A registry of named pattern definitions with O(log n) lookup.
pub struct PatternRegistry {
    inner: BTreeMap<String, PatternDefinition>,
}

impl PatternRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    /// Insert a pattern into the registry.
    pub fn insert(&mut self, pattern: PatternDefinition) {
        self.inner.insert(pattern.name.clone(), pattern);
    }

    /// Look up a pattern by name.
    pub fn get(&self, name: &str) -> Option<&PatternDefinition> {
        self.inner.get(name)
    }

    /// Get all patterns in deterministic (alphabetical) order.
    pub fn all(&self) -> Vec<&PatternDefinition> {
        self.inner.values().collect()
    }

    /// Get all pattern names in deterministic (alphabetical) order.
    pub fn names(&self) -> Vec<&str> {
        self.inner.keys().map(|s| s.as_str()).collect()
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

            let pattern = parse_pattern(file.contents());
            tracing::trace!(
                name = %pattern.name,
                category = %pattern.category,
                entity_type = %pattern.entity_type,
                has_validator = pattern.validate.is_some(),
                "pattern loaded",
            );
            reg.insert(pattern);
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

/// Look up a built-in pattern by name.
pub fn get_pattern(name: &str) -> Option<&'static PatternDefinition> {
    BUILTIN_REGISTRY.get(name)
}

/// Get all built-in patterns in deterministic (alphabetical) order.
pub fn get_all_patterns() -> Vec<&'static PatternDefinition> {
    BUILTIN_REGISTRY.all()
}

/// Get all built-in pattern names in deterministic (alphabetical) order.
pub fn get_all_pattern_names() -> Vec<&'static str> {
    BUILTIN_REGISTRY.names()
}

/// Get a reference to the built-in [`PatternRegistry`].
pub fn builtin_registry() -> &'static PatternRegistry {
    &BUILTIN_REGISTRY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_all_patterns() {
        let patterns = get_all_patterns();
        assert!(!patterns.is_empty());
        assert!(patterns.len() >= 10);
    }

    #[test]
    fn pattern_names_are_sorted() {
        let names = get_all_pattern_names();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn get_known_pattern() {
        let ssn = get_pattern("ssn").unwrap();
        assert_eq!(ssn.name, "ssn");
        assert_eq!(ssn.category, EntityCategory::Pii);
        assert_eq!(ssn.entity_type, "government_id");
        assert!(ssn.validate.is_some());
        assert!(ssn.confidence > 0.0);
    }

    #[test]
    fn get_unknown_pattern_returns_none() {
        assert!(get_pattern("nonexistent").is_none());
    }

    #[test]
    fn all_patterns_have_nonempty_fields() {
        for p in get_all_patterns() {
            assert!(!p.name.is_empty(), "pattern name is empty");
            assert!(!p.entity_type.is_empty(), "entity_type is empty for {}", p.name);
            assert!(!p.pattern_str.is_empty(), "pattern_str is empty for {}", p.name);
            assert!(p.confidence > 0.0, "confidence is 0 for {}", p.name);
            assert!(p.confidence <= 1.0, "confidence > 1 for {}", p.name);
        }
    }

    #[test]
    fn all_patterns_compile_as_regex() {
        for p in get_all_patterns() {
            assert!(
                regex::Regex::new(&p.pattern_str).is_ok(),
                "pattern {} failed to compile: {}",
                p.name,
                p.pattern_str,
            );
        }
    }

    #[test]
    fn ssn_pattern_has_validator() {
        let ssn = get_pattern("ssn").unwrap();
        let validate = ssn.validate.unwrap();
        assert!(validate("123-45-6789"));
        assert!(!validate("000-00-0000"));
    }

    #[test]
    fn credit_card_pattern_has_luhn_validator() {
        let cc = get_pattern("credit-card").unwrap();
        assert_eq!(cc.category, EntityCategory::Financial);
        let validate = cc.validate.unwrap();
        assert!(validate("4539 1488 0343 6467"));
    }

    #[test]
    fn pattern_categories_are_correct() {
        assert_eq!(get_pattern("email").unwrap().category, EntityCategory::Pii);
        assert_eq!(get_pattern("aws-key").unwrap().category, EntityCategory::Credentials);
        assert_eq!(get_pattern("credit-card").unwrap().category, EntityCategory::Financial);
    }

    #[test]
    fn entity_types_match_entity_kind_slugs() {
        assert_eq!(get_pattern("ssn").unwrap().entity_type, "government_id");
        assert_eq!(get_pattern("email").unwrap().entity_type, "email_address");
        assert_eq!(get_pattern("phone").unwrap().entity_type, "phone_number");
        assert_eq!(get_pattern("credit-card").unwrap().entity_type, "payment_card");
        assert_eq!(get_pattern("aws-key").unwrap().entity_type, "api_key");
        assert_eq!(get_pattern("github-token").unwrap().entity_type, "auth_token");
        assert_eq!(get_pattern("ipv4").unwrap().entity_type, "ip_address");
    }

    #[test]
    fn pattern_definition_is_debug_and_clone() {
        let p = get_pattern("ssn").unwrap();
        let cloned = p.clone();
        assert_eq!(cloned.name, p.name);
        let _ = format!("{:?}", p);
    }

    #[test]
    fn no_duplicate_pattern_names() {
        let all = get_all_patterns();
        let names: Vec<_> = all.iter().map(|p| &p.name).collect();
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
        let mut reg = PatternRegistry::new();
        reg.insert(PatternDefinition {
            name: "test".into(),
            category: EntityCategory::Pii,
            entity_type: "test".into(),
            pattern_str: r"\d+".into(),
            confidence: 0.9,
            validate: None,
        });

        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());
        assert_eq!(reg.get("test").unwrap().name, "test");
    }
}

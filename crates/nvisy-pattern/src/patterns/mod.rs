//! Built-in regex pattern definitions and validation helpers.
//!
//! Patterns are loaded at startup from the embedded `assets/patterns.json`
//! file and compiled into a static registry keyed by pattern name.

/// Checksum and format validators used by pattern definitions.
pub mod validators;

use std::collections::HashMap;
use std::sync::LazyLock;

use nvisy_ontology::ontology::entity::EntityCategory;

/// JSON representation of a pattern loaded from disk.
#[derive(Debug, Clone, serde::Deserialize)]
struct PatternJson {
    /// Human-readable pattern name (used as the registry key).
    name: String,
    /// Category string (e.g. `"pii"`, `"phi"`, `"financial"`).
    category: String,
    /// The entity type tag emitted when this pattern matches.
    entity_type: String,
    /// The regex pattern string.
    pattern: String,
    /// Base confidence score assigned to matches.
    confidence: f64,
    /// Optional validator name resolved at load time (e.g. `"ssn"`, `"luhn"`).
    #[serde(default)]
    validator: Option<String>,
}

/// A compiled regex-based detection pattern with optional post-match validation.
pub struct PatternDefinition {
    /// Unique name identifying this pattern in the registry.
    pub name: String,
    /// The entity category (PII, PHI, Financial, etc.).
    pub category: EntityCategory,
    /// The entity type tag emitted on match (e.g. `"ssn"`, `"credit_card"`).
    pub entity_type: String,
    /// The raw regex pattern string.
    pub pattern_str: String,
    /// Base confidence score assigned to matches of this pattern.
    pub confidence: f64,
    /// Optional validation function applied after a regex match succeeds.
    pub validate: Option<fn(&str) -> bool>,
}

/// Maps a category string from `patterns.json` to its [`EntityCategory`] variant.
fn parse_category(s: &str) -> EntityCategory {
    match s {
        "pii" => EntityCategory::Pii,
        "phi" => EntityCategory::Phi,
        "financial" => EntityCategory::Financial,
        "credentials" => EntityCategory::Credentials,
        _ => EntityCategory::Custom,
    }
}

/// Resolves a validator name string to its corresponding validation function.
fn resolve_validator(name: &str) -> Option<fn(&str) -> bool> {
    match name {
        "ssn" => Some(validators::validate_ssn),
        "luhn" => Some(validators::luhn_check),
        _ => None,
    }
}

/// Deserializes and compiles all patterns from the embedded `patterns.json` asset.
fn load_patterns() -> Vec<PatternDefinition> {
    let json_bytes = include_bytes!("../../assets/patterns.json");
    let raw: Vec<PatternJson> =
        serde_json::from_slice(json_bytes).expect("Failed to parse patterns.json");

    raw.into_iter()
        .map(|p| PatternDefinition {
            category: parse_category(&p.category),
            validate: p.validator.as_deref().and_then(resolve_validator),
            name: p.name,
            entity_type: p.entity_type,
            pattern_str: p.pattern,
            confidence: p.confidence,
        })
        .collect()
}

static PATTERNS: LazyLock<Vec<PatternDefinition>> = LazyLock::new(load_patterns);

static REGISTRY: LazyLock<HashMap<&'static str, &'static PatternDefinition>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();
        for p in PATTERNS.iter() {
            map.insert(p.name.as_str(), p);
        }
        map
    });

/// Look up a built-in pattern by name.
pub fn get_pattern(name: &str) -> Option<&'static PatternDefinition> {
    REGISTRY.get(name).copied()
}

/// Get all built-in patterns.
pub fn get_all_patterns() -> Vec<&'static PatternDefinition> {
    REGISTRY.values().copied().collect()
}

/// Get all built-in pattern names.
pub fn get_all_pattern_names() -> Vec<&'static str> {
    REGISTRY.keys().copied().collect()
}

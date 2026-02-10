pub mod validators;

use std::collections::HashMap;
use std::sync::LazyLock;

use nvisy_core::types::EntityCategory;

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

/// Definition of a regex-based detection pattern.
pub struct PatternDefinition {
    pub name: String,
    pub category: EntityCategory,
    pub entity_type: String,
    pub pattern_str: String,
    pub confidence: f64,
    pub validate: Option<fn(&str) -> bool>,
}

fn parse_category(s: &str) -> EntityCategory {
    match s {
        "pii" => EntityCategory::Pii,
        "phi" => EntityCategory::Phi,
        "financial" => EntityCategory::Financial,
        "credentials" => EntityCategory::Credentials,
        _ => EntityCategory::Custom,
    }
}

fn resolve_validator(name: &str) -> Option<fn(&str) -> bool> {
    match name {
        "ssn" => Some(validators::validate_ssn),
        "luhn" => Some(validators::luhn_check),
        _ => None,
    }
}

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

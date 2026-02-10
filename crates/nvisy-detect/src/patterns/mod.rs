pub mod api_key;
pub mod credit_card;
pub mod email;
pub mod ip_address;
pub mod phone;
pub mod ssn;

use nvisy_core::types::EntityCategory;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Definition of a regex-based detection pattern.
pub struct PatternDefinition {
    pub name: &'static str,
    pub category: EntityCategory,
    pub entity_type: &'static str,
    pub pattern_str: &'static str,
    pub confidence: f64,
    pub validate: Option<fn(&str) -> bool>,
}

static REGISTRY: LazyLock<HashMap<&'static str, &'static PatternDefinition>> = LazyLock::new(|| {
    let patterns: &[&'static PatternDefinition] = &[
        &ssn::SSN_PATTERN,
        &email::EMAIL_PATTERN,
        &phone::PHONE_PATTERN,
        &credit_card::CREDIT_CARD_PATTERN,
        &api_key::AWS_KEY_PATTERN,
        &api_key::GITHUB_TOKEN_PATTERN,
        &api_key::STRIPE_KEY_PATTERN,
        &api_key::GENERIC_KEY_PATTERN,
        &ip_address::IPV4_PATTERN,
        &ip_address::IPV6_PATTERN,
    ];
    let mut map = HashMap::new();
    for p in patterns {
        map.insert(p.name, *p);
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

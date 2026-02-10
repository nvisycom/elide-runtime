use nvisy_core::types::EntityCategory;

use super::PatternDefinition;

pub static IPV4_PATTERN: PatternDefinition = PatternDefinition {
    name: "ipv4",
    category: EntityCategory::Pii,
    entity_type: "ip_address",
    pattern_str: r"\b(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\b",
    confidence: 0.75,
    validate: None,
};

pub static IPV6_PATTERN: PatternDefinition = PatternDefinition {
    name: "ipv6",
    category: EntityCategory::Pii,
    entity_type: "ip_address",
    pattern_str: r"\b(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}\b|(?:[0-9a-fA-F]{1,4}:){1,7}:|::(?:[0-9a-fA-F]{1,4}:){0,5}[0-9a-fA-F]{1,4}\b",
    confidence: 0.75,
    validate: None,
};

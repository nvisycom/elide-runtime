use nvisy_core::types::EntityCategory;

use super::PatternDefinition;

pub static EMAIL_PATTERN: PatternDefinition = PatternDefinition {
    name: "email",
    category: EntityCategory::Pii,
    entity_type: "email",
    pattern_str: r"\b[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}\b",
    confidence: 0.95,
    validate: None,
};

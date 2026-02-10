use nvisy_core::types::EntityCategory;

use super::PatternDefinition;

pub static PHONE_PATTERN: PatternDefinition = PatternDefinition {
    name: "phone",
    category: EntityCategory::Pii,
    entity_type: "phone",
    pattern_str: r"(?:\+\d{1,3}[\s.\-]?)?\(?\d{2,4}\)?[\s.\-]?\d{3,4}[\s.\-]?\d{4}\b",
    confidence: 0.8,
    validate: None,
};

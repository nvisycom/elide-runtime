use nvisy_core::types::EntityCategory;

use super::PatternDefinition;

fn validate_ssn(value: &str) -> bool {
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    let area: u32 = match parts[0].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let group: u32 = match parts[1].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let serial: u32 = match parts[2].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    area > 0 && area < 900 && area != 666 && group > 0 && serial > 0
}

pub static SSN_PATTERN: PatternDefinition = PatternDefinition {
    name: "ssn",
    category: EntityCategory::Pii,
    entity_type: "ssn",
    pattern_str: r"\b(\d{3})-(\d{2})-(\d{4})\b",
    confidence: 0.9,
    validate: Some(validate_ssn),
};

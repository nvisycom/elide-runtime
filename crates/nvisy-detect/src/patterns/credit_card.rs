use nvisy_core::types::EntityCategory;

use super::PatternDefinition;

/// Luhn check algorithm for credit card validation.
pub fn luhn_check(num: &str) -> bool {
    let digits: String = num.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return false;
    }
    let mut sum = 0u32;
    let mut alternate = false;
    for ch in digits.chars().rev() {
        let mut n = ch.to_digit(10).unwrap_or(0);
        if alternate {
            n *= 2;
            if n > 9 {
                n -= 9;
            }
        }
        sum += n;
        alternate = !alternate;
    }
    sum % 10 == 0
}

fn validate_credit_card(value: &str) -> bool {
    luhn_check(value)
}

pub static CREDIT_CARD_PATTERN: PatternDefinition = PatternDefinition {
    name: "credit-card",
    category: EntityCategory::Financial,
    entity_type: "credit_card",
    pattern_str: r"\b(?:\d[ \-]*?){13,19}\b",
    confidence: 0.85,
    validate: Some(validate_credit_card),
};

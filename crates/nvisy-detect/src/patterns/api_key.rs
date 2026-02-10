use nvisy_core::types::EntityCategory;

use super::PatternDefinition;

pub static AWS_KEY_PATTERN: PatternDefinition = PatternDefinition {
    name: "aws-key",
    category: EntityCategory::Credentials,
    entity_type: "aws_access_key",
    pattern_str: r"\bAKIA[0-9A-Z]{16}\b",
    confidence: 0.95,
    validate: None,
};

pub static GITHUB_TOKEN_PATTERN: PatternDefinition = PatternDefinition {
    name: "github-token",
    category: EntityCategory::Credentials,
    entity_type: "github_token",
    pattern_str: r"\bgh[pousr]_[a-zA-Z0-9]{36}\b",
    confidence: 0.95,
    validate: None,
};

pub static STRIPE_KEY_PATTERN: PatternDefinition = PatternDefinition {
    name: "stripe-key",
    category: EntityCategory::Credentials,
    entity_type: "stripe_key",
    pattern_str: r"\bsk_(live|test)_[a-zA-Z0-9]{24,}\b",
    confidence: 0.95,
    validate: None,
};

pub static GENERIC_KEY_PATTERN: PatternDefinition = PatternDefinition {
    name: "generic-api-key",
    category: EntityCategory::Credentials,
    entity_type: "api_key",
    pattern_str: r#"(?i)(?:api[_\-]?key|api[_\-]?secret|access[_\-]?token|secret[_\-]?key|bearer)\s*[:=]\s*["']?([a-zA-Z0-9_\-]{20,})["']?"#,
    confidence: 0.7,
    validate: None,
};

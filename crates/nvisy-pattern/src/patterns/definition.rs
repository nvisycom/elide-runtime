//! Pattern definition type.

use nvisy_core::data::EntityCategory;

/// A compiled regex-based detection pattern with optional post-match validation.
#[derive(Debug, Clone)]
pub struct PatternDefinition {
    /// Unique name identifying this pattern in the registry.
    pub name: String,
    /// The entity category (PII, PHI, Financial, etc.).
    pub category: EntityCategory,
    /// The entity type tag emitted on match (e.g. `"government_id"`, `"payment_card"`).
    pub entity_type: String,
    /// The raw regex pattern string.
    pub pattern_str: String,
    /// Base confidence score assigned to matches of this pattern.
    pub confidence: f64,
    /// Optional validation function applied after a regex match succeeds.
    pub validate: Option<fn(&str) -> bool>,
}

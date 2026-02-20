//! Pattern trait and type-erased alias.

use nvisy_core::data::{EntityCategory, EntityKind};

/// A named detection pattern with category, entity kind, regex, confidence,
/// and optional post-match validation.
pub trait Pattern: Send + Sync {
    /// Unique name identifying this pattern in the registry.
    fn name(&self) -> &str;

    /// The entity category (PII, PHI, Financial, etc.).
    fn category(&self) -> &EntityCategory;

    /// The type-safe entity kind.
    fn entity_kind(&self) -> EntityKind;

    /// The raw regex pattern string.
    fn pattern_str(&self) -> &str;

    /// Base confidence score assigned to matches of this pattern.
    fn confidence(&self) -> f64;

    /// Run the post-match validator, if one is configured.
    ///
    /// Returns `true` when there is no validator (i.e. unconditional pass).
    fn validate(&self, value: &str) -> bool;

    /// Whether this pattern has a post-match validator.
    fn has_validator(&self) -> bool;
}

/// Type-erased pattern.
pub type BoxPattern = Box<dyn Pattern>;

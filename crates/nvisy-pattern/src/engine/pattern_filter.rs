//! [`PatternFilter`]: tag-based narrowing of the active pattern set.
//!
//! Sits between [`PatternEngineBuilder::with_filter`] and the
//! per-pattern [`PatternMetadata`]: a filter selects only those
//! patterns whose metadata satisfies every non-empty constraint.
//!
//! [`PatternEngineBuilder::with_filter`]: super::PatternEngineBuilder::with_filter
//! [`PatternMetadata`]: crate::patterns::PatternMetadata

use nvisy_ontology::primitive::LanguageTag;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Tag-based narrowing of the active pattern set.
///
/// A pattern passes the filter when every non-empty constraint is
/// satisfied (AND across fields); within a single field, the
/// pattern's metadata either overlaps the filter (OR within field)
/// **or** is empty on that axis (the pattern is considered universal
/// on that dimension and passes any filter).
///
/// Empty fields on the filter are unconstrained. An entirely empty
/// filter selects every pattern.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PatternFilter {
    /// Languages the pattern must list as applicable (OR within field).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<String>")]
    pub languages: Vec<LanguageTag>,
    /// Industries the pattern must list (OR within field).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub industries: Vec<String>,
    /// Regions the pattern must list (OR within field).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<String>,
    /// Compliance regimes the pattern must list (OR within field).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compliance: Vec<String>,
}

impl PatternFilter {
    /// Whether every constraint is empty (the filter selects everything).
    pub fn is_unconstrained(&self) -> bool {
        self.languages.is_empty()
            && self.industries.is_empty()
            && self.regions.is_empty()
            && self.compliance.is_empty()
    }
}

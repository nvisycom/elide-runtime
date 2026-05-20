//! Detection node configuration.
//!
//! [`Detection`] runs at **phase 2**, after extraction has converted
//! raw content into text. Both methods (NER and pattern) always run;
//! the user controls *how* they run via optional settings. Their
//! outputs are merged in the subsequent [`Deduplication`] phase.
//!
//! NER and pattern detection are independent and can execute
//! concurrently within the same phase.
//!
//! [`Deduplication`]: crate::workflow::Deduplication

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::entity::EntityKind;
use crate::primitive::LanguageTag;

/// NER detection settings (LLM-based named entity recognition).
///
/// Controls which entity kinds are targeted and sets the minimum
/// confidence threshold below which detections are discarded.
#[derive(Debug, Clone, Default, PartialEq, Validate)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NerDetection {
    /// Entity kinds to detect. An empty list means all known kinds.
    #[serde(default)]
    pub entity_kinds: Vec<EntityKind>,
    /// Minimum confidence threshold for detections (0.0 to 1.0).
    /// When `None`, confidence filtering is disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub confidence_threshold: Option<f64>,
    /// Run a second LLM pass to adjust confidence based on surrounding
    /// document context. Requires an LLM provider to be configured.
    #[serde(default)]
    pub contextual_adjustment: bool,
}

/// Pattern detection settings (regex, checksum, dictionary).
///
/// Controls which patterns run and what confidence threshold applies.
#[derive(Debug, Clone, Default, PartialEq, Validate)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct PatternDetection {
    /// Restrict detection to the named patterns only. When empty, all
    /// built-in patterns are used.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patterns: Vec<String>,
    /// Minimum confidence threshold for detections (0.0 to 1.0).
    /// When `None`, the engine's default threshold applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub confidence_threshold: Option<f64>,
    /// Narrow the active patterns (regex and dictionary alike) by
    /// their declared tags.
    ///
    /// Patterns whose metadata leaves a tag field empty are considered
    /// **universal** on that axis — they pass any filter for that
    /// field. Patterns ship with empty metadata by default, so an
    /// untagged pattern always passes any filter.
    ///
    /// When `None`, all patterns are eligible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<PatternFilter>,
}

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

/// Unified entity detection configuration.
///
/// Both NER and pattern detection always run. Settings here
/// customize their behavior; `None` means default settings.
/// NER and pattern detection are independent and execute
/// concurrently within the detection phase.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Detection {
    /// NER detection settings. `None` = defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ner: Option<NerDetection>,
    /// Pattern detection settings. `None` = defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<PatternDetection>,
}

impl Detection {
    /// Validates all configured sub-methods.
    pub fn validate(&self) -> Result<(), validator::ValidationErrors> {
        if let Some(ref ner) = self.ner {
            ner.validate()?;
        }
        if let Some(ref pattern) = self.pattern {
            pattern.validate()?;
        }
        Ok(())
    }
}

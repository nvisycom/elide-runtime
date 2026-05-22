//! [`PatternDetection`]: graph-config shape accepted by
//! [`PatternRecognizer::from_config`].
//!
//! The config schema for pattern-based detection lives next to its
//! sole consumer rather than alongside the pattern engine, so
//! `nvisy-pattern` stays a pure runtime crate (no workflow-config
//! types). The `filter` field is still typed against
//! [`PatternFilter`] from `nvisy-pattern`, since the filter is the
//! engine builder's input type.
//!
//! [`PatternRecognizer::from_config`]: super::PatternRecognizer::from_config

use nvisy_pattern::PatternFilter;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

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

//! Redaction config: the `[redaction]` config section + the
//! per-request [`Redaction`] plan node.

mod plan;

use nvisy_core::primitive::ConfidenceThreshold;
use serde::{Deserialize, Serialize};

pub use self::plan::Redaction;

/// `[redaction]` config section: deployment-wide fallback values
/// for the plan [`Redaction`] node's optional fields.
///
/// Matches the `*Config` naming used by
/// [`EngineConfig`],
/// [`ExtractionConfig`],
/// [`DetectionConfig`] so all four
/// [`RuntimeConfig`] subsystem
/// sections share one shape: a struct deserialized from one TOML
/// section, built once at engine startup, shared across runs via
/// `Arc`.
///
/// [`EngineConfig`]: crate::pipeline::EngineConfig
/// [`ExtractionConfig`]: crate::pipeline::ExtractionConfig
/// [`DetectionConfig`]: crate::pipeline::DetectionConfig
/// [`RuntimeConfig`]: crate::pipeline::RuntimeConfig
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionConfig {
    /// Default minimum confidence threshold for entities that
    /// don't match a policy rule. Plan
    /// [`Redaction::confidence_threshold`] overrides this when set.
    /// Defaults to `0.5`.
    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: ConfidenceThreshold,
    /// Default for whether to strip embedded document metadata
    /// (EXIF, PDF properties). Plan [`Redaction::process_metadata`]
    /// overrides this when set. Defaults to `false`.
    #[serde(default)]
    pub process_metadata: bool,
}

impl Default for RedactionConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: default_confidence_threshold(),
            process_metadata: false,
        }
    }
}

fn default_confidence_threshold() -> ConfidenceThreshold {
    ConfidenceThreshold::clamped(0.5)
}

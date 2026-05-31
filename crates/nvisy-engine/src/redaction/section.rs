//! [`RedactionSection`]: the `[redaction]` config section.
//!
//! Matches the `*Section` naming used by [`EngineSection`],
//! [`ExtractionSection`], [`DetectionSection`] so all four
//! `RuntimeConfig` subsystem sections share one shape: a struct
//! deserialized from one TOML section, built once at engine
//! startup, shared across runs via `Arc`.
//!
//! The workflow [`Redaction`] node carries per-workflow knobs as
//! optional fields; this section supplies the values used when those
//! fields are unset.
//!
//! [`Redaction`]: crate::redaction::Redaction
//! [`EngineSection`]: crate::pipeline::EngineSection
//! [`ExtractionSection`]: crate::extraction::ExtractionSection
//! [`DetectionSection`]: crate::detection::DetectionSection

use nvisy_ontology::primitive::ConfidenceThreshold;
use serde::{Deserialize, Serialize};

/// `[redaction]` config section: deployment-wide fallback values for
/// the workflow [`Redaction`] node's optional fields.
///
/// [`Redaction`]: crate::redaction::Redaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionSection {
    /// Default minimum confidence threshold for entities that
    /// don't match a policy rule. Workflow `Redaction.confidence_threshold`
    /// overrides this when set. Defaults to `0.5`.
    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: ConfidenceThreshold,
    /// Default for whether to strip embedded document metadata
    /// (EXIF, PDF properties). Workflow `Redaction.process_metadata`
    /// overrides this when set. Defaults to `false`.
    #[serde(default)]
    pub process_metadata: bool,
}

impl Default for RedactionSection {
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

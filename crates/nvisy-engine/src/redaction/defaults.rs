//! [`RedactionDefaults`]: server-wide redaction fallbacks.
//!
//! The workflow [`Redaction`] node carries per-workflow knobs as
//! optional fields; this struct supplies the values used when those
//! fields are unset.
//!
//! [`Redaction`]: crate::redaction::Redaction

use nvisy_ontology::primitive::ConfidenceThreshold;
use serde::{Deserialize, Serialize};

/// `[redactor]` config section: workflow-wide fallback defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionDefaults {
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

impl Default for RedactionDefaults {
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

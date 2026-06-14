//! `[redaction]` config section: deployment-wide fallback values
//! for the per-plan [`Redaction`] node's optional fields.
//!
//! Matches the `*Config` naming used by [`EngineConfig`],
//! [`ExtractionConfig`], [`DetectionConfig`] so every
//! [`RuntimeConfig`] subsystem section shares one shape: a struct
//! deserialized from one TOML section, built once at engine
//! startup, shared across runs via `Arc`.
//!
//! Per-request redaction plan knobs ([`Redaction`], [`Validation`])
//! live in [`super::plan`].
//!
//! [`Redaction`]: super::plan::Redaction
//! [`Validation`]: super::plan::Validation
//! [`EngineConfig`]: crate::core::EngineConfig
//! [`ExtractionConfig`]: crate::detection::ExtractionConfig
//! [`DetectionConfig`]: crate::detection::DetectionConfig
//! [`RuntimeConfig`]: crate::core::RuntimeConfig

use nvisy_core::primitive::ConfidenceThreshold;
use serde::{Deserialize, Serialize};

use crate::policy::redaction::ModalityRedactions;

/// Deployment-wide redaction defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionConfig {
    /// Default minimum confidence threshold for entities that
    /// don't match a policy rule. The plan
    /// [`Redaction::confidence_threshold`][rct] overrides this
    /// when set. Defaults to `0.5`.
    ///
    /// [rct]: super::plan::Redaction::confidence_threshold
    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: ConfidenceThreshold,
    /// Default for whether to strip embedded document metadata
    /// (EXIF, PDF properties). The plan
    /// [`Redaction::process_metadata`][rpm] overrides this when
    /// set. Defaults to `false`.
    ///
    /// [rpm]: super::plan::Redaction::process_metadata
    #[serde(default)]
    pub process_metadata: bool,
    /// Deployment-wide fallback operators: when a policy rule's
    /// [`Action::Redact`][crate::policy::Action::Redact] doesn't
    /// cover the entity's modality, the redaction phase falls
    /// back to whatever is set here for that modality. Missing
    /// entries at both levels cause the rule to silently skip —
    /// operators should be configured at one of the two layers.
    #[serde(default)]
    pub default_operators: ModalityRedactions,
}

impl Default for RedactionConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: default_confidence_threshold(),
            process_metadata: false,
            default_operators: ModalityRedactions::default(),
        }
    }
}

fn default_confidence_threshold() -> ConfidenceThreshold {
    ConfidenceThreshold::clamped(0.5)
}

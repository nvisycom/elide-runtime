//! [`RedactionDefaults`]: server-wide redaction fallbacks.
//!
//! The workflow [`Redaction`] node carries per-workflow knobs as
//! optional fields; this struct supplies the values used when those
//! fields are unset.
//!
//! Also houses the redaction-side TTS config — TTS is used by the
//! audio redaction codec to synthesize replacement audio for
//! redacted spans, so it lives here rather than as a separate
//! generation phase.
//!
//! [`Redaction`]: crate::redaction::Redaction

use nvisy_ontology::primitive::ConfidenceThreshold;
use serde::{Deserialize, Serialize};

use super::tts::RedactorTtsConfig;

/// `[redactor]` config section: workflow-wide fallback defaults
/// plus the audio redaction TTS config.
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
    /// TTS provider config used by the audio redaction codec to
    /// synthesize replacement audio. `None` falls back to non-TTS
    /// audio replacement (silence, beep).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tts: Option<RedactorTtsConfig>,
}

impl Default for RedactionDefaults {
    fn default() -> Self {
        Self {
            confidence_threshold: default_confidence_threshold(),
            process_metadata: false,
            tts: None,
        }
    }
}

fn default_confidence_threshold() -> ConfidenceThreshold {
    ConfidenceThreshold::clamped(0.5)
}

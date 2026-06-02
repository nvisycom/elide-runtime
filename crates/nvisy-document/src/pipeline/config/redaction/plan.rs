//! Redaction node plan.
//!
//! [`Redaction`] runs at **phase 4**, after deduplication has
//! produced a final scored entity list. It applies redaction
//! instructions to the document envelope, replacing or removing
//! detected values, and optionally strips embedded document
//! metadata.

use nvisy_core::primitive::ConfidenceThreshold;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Per-plan knobs for the redaction phase.
///
/// Unset fields fall back to `[redaction]` defaults in `Nvisy.toml`
/// ([`RedactionConfig`]); if neither is set,
/// hard-coded defaults apply (0.5 threshold, no metadata stripping).
///
/// [`RedactionConfig`]: super::RedactionConfig
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Redaction {
    /// Minimum confidence threshold for default redaction (0.0 to 1.0).
    /// Entities below this threshold that don't match a policy rule
    /// are skipped. `None` falls back to `[redaction].confidence_threshold`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_threshold: Option<ConfidenceThreshold>,
    /// Strip or redact document metadata (EXIF, PDF properties).
    /// `None` falls back to `[redaction].process_metadata`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_metadata: Option<bool>,
}

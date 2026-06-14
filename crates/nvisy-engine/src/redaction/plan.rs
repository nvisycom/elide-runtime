//! Per-pass plan and per-phase knobs the redaction pipeline reads
//! against each imported document.
//!
//! Three concerns share this file:
//!
//! - [`RedactionPlan`] — top-level per-request bundle the
//!   redaction pipeline reads once per document.
//! - [`Redaction`] — per-plan knobs for the redaction phase
//!   itself (confidence threshold override, metadata handling).
//! - [`Validation`] — per-plan knobs for the post-redaction leak
//!   check phase.

use nvisy_core::primitive::ConfidenceThreshold;
use nvisy_toolkit::validation::Severity;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Per-request bundle of redaction-side phase configs.
///
/// The redaction pipeline reads this once per document and routes
/// each phase (redaction, then validation) to the matching field.
#[derive(Debug, Clone, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct RedactionPlan {
    /// Redaction settings applied after policy evaluation.
    pub redaction: Redaction,
    /// Validation settings for the post-redaction leak check.
    pub validation: Validation,
}

/// Per-plan knobs for the redaction phase.
///
/// Unset fields fall back to `[redaction]` defaults in
/// `Nvisy.toml` ([`RedactionConfig`]); if neither is set,
/// hard-coded defaults apply (0.5 threshold, no metadata
/// stripping).
///
/// [`RedactionConfig`]: super::RedactionConfig
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Redaction {
    /// Minimum confidence threshold for default redaction (0.0 to
    /// 1.0). Entities below this threshold that don't match a
    /// policy rule are skipped. `None` falls back to
    /// `[redaction].confidence_threshold`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_threshold: Option<ConfidenceThreshold>,
    /// Strip or redact document metadata (EXIF, PDF properties).
    /// `None` falls back to `[redaction].process_metadata`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_metadata: Option<bool>,
}

/// Per-plan validation settings.
///
/// `leak_severity` controls what the phase does when the canonical
/// [`LeakCheck`] finds a value that should have been redacted but
/// still appears in the output:
///
/// - [`Severity::Warn`] (default) — log the leak and continue.
/// - [`Severity::Fail`] — fail the pass with a validation error
///   listing the leaked values.
///
/// [`LeakCheck`]: nvisy_toolkit::validation::LeakCheck
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Validation {
    /// Severity stamped onto every [`Finding`] emitted by the
    /// canonical leak check.
    ///
    /// [`Finding`]: nvisy_toolkit::validation::Finding
    #[serde(default)]
    pub leak_severity: Severity,
}

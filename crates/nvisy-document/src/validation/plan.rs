//! Validation phase configuration.
//!
//! [`Validation`] runs after redaction. It re-scans the redacted
//! output to verify that no originally detected values remain
//! visible, optionally failing the pipeline run when leaks are found.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Severity;

/// Per-plan validation settings.
///
/// `leak_severity` controls what the phase does when the canonical
/// [`LeakCheck`] finds a value that should have been redacted but
/// still appears in the output:
///
/// - [`Severity::Warn`] (default) — log the leak and continue. The
///   run succeeds.
/// - [`Severity::Fail`] — log the leak and fail the run with a
///   validation error listing the leaked values.
///
/// [`LeakCheck`]: crate::validation::LeakCheck
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Validation {
    /// Severity stamped onto every [`Finding`] emitted by the
    /// canonical leak check.
    ///
    /// [`Finding`]: crate::validation::Finding
    #[serde(default)]
    pub leak_severity: Severity,
}

//! Per-plan validation phase configuration.

use nvisy_toolkit::validation::Severity;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

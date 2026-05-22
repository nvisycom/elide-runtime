//! Per-phase timeout policy used by [`crate::pipeline::EngineInput`].

use super::TimeoutPolicy;

/// Per-phase timeout override.
///
/// Carried by the phases that involve external calls (extraction,
/// detection, redaction). Wraps the entire phase with a wall-clock
/// deadline.
#[derive(Debug, Clone, Default)]
pub struct PhasePolicy {
    /// Timeout policy wrapping the entire phase.
    pub timeout: Option<TimeoutPolicy>,
}

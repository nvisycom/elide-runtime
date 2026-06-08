//! [`DetectionStatus`]: lifecycle states for a detection pass.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Lifecycle states for a single detection pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DetectionStatus {
    /// Submitted, not yet started.
    Pending,
    /// Imports loaded; recognisers / evaluators running.
    Running,
    /// All documents processed without error.
    Succeeded,
    /// Some documents detected entities; others errored.
    PartialFailure,
    /// Every document errored or the pass failed before any
    /// document was processed.
    Failed,
    /// The caller cancelled the detection pass before it finished.
    Cancelled,
}

impl DetectionStatus {
    /// `true` for [`Self::Succeeded`] / [`Self::PartialFailure`] /
    /// [`Self::Failed`] / [`Self::Cancelled`] — anything past the
    /// running phase.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::PartialFailure | Self::Failed | Self::Cancelled
        )
    }
}

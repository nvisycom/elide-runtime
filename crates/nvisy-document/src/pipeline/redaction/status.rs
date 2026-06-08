//! [`RedactionStatus`]: lifecycle states for a redaction pass.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Lifecycle states for a single redaction pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RedactionStatus {
    /// Submitted, not yet started.
    Pending,
    /// Apply / validate / export running.
    Running,
    /// All documents redacted, validated, and exported.
    Succeeded,
    /// Some documents redacted while others failed.
    PartialFailure,
    /// Every document errored or the pass failed before any
    /// document was processed.
    Failed,
    /// The caller cancelled the redaction pass before it finished.
    Cancelled,
}

impl RedactionStatus {
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

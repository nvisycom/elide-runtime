//! Per-source redaction summary.

use serde::{Deserialize, Serialize};

use nvisy_core::path::ContentSource;

/// Summary of redactions applied to a single content source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
pub struct RedactionSummary {
    /// The content source these counts apply to.
    #[serde(flatten)]
    pub source: ContentSource,
    /// Number of redactions successfully applied.
    pub redactions_applied: usize,
    /// Number of redactions skipped (e.g. due to review holds or errors).
    pub redactions_skipped: usize,
}

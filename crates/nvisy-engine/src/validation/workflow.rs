//! Validation phase configuration.
//!
//! [`Validation`] runs after redaction. It re-scans the redacted
//! output to verify that no originally detected values remain
//! visible, optionally failing the pipeline run if any leaks are
//! found.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Controls how the post-redaction leak check affects the overall pipeline
/// outcome.
///
/// [`Validation`]: crate::validation::Validation
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Validation {
    /// Fail the run if any leaked values are detected.
    #[serde(default)]
    pub fail_on_leak: bool,
}

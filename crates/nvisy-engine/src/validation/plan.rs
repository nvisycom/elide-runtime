//! Validation phase configuration.
//!
//! [`Validation`] runs after redaction. It re-scans the redacted
//! output to verify that no originally detected values remain
//! visible, optionally failing the pipeline run if any leaks are
//! found.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How the validation phase reacts when post-redaction re-scan finds
/// a value that should have been redacted but still appears in the
/// output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OnLeak {
    /// Log the leak and continue. The run still succeeds.
    #[default]
    Ignore,
    /// Fail the run with a validation error listing the leaked values.
    Fail,
}

/// Controls how the post-redaction leak check affects the overall pipeline
/// outcome.
///
/// [`Validation`]: crate::validation::Validation
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Validation {
    /// What to do when the re-scan finds a leaked value.
    #[serde(default)]
    pub on_leak: OnLeak,
}

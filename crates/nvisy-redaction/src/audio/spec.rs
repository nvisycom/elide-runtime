//! Audio redaction specification.

use serde::{Deserialize, Serialize};

/// Audio redaction specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AudioRedactionSpec {
    /// Replace with silence.
    Silence,
    /// Remove the segment entirely.
    Remove,
    /// Replace with synthetic audio.
    Synthesize,
}

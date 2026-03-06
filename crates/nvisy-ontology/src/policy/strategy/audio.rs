//! Audio redaction strategies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Audio redaction strategy.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AudioRedactionStrategy {
    /// Replace with silence.
    Silence,
    /// Remove the segment entirely.
    Remove,
}

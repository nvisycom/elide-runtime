//! Temporal interval type.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A time interval within an audio or video stream.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TimeSpan {
    /// Start time in seconds from the beginning of the stream.
    pub start_secs: f64,
    /// End time in seconds from the beginning of the stream.
    pub end_secs: f64,
}

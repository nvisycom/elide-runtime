//! Audio redaction strategies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Audio redaction strategy.
///
/// The [`Default`] impl returns [`Silence`] — preserves duration while
/// removing the audible content.
///
/// [`Silence`]: AudioStrategy::Silence
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AudioStrategy {
    /// Replace with silence.
    #[default]
    Silence,
    /// Remove the segment entirely.
    Remove,
}

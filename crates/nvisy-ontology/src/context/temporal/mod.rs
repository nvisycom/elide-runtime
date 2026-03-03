//! Date and time-based reference data.

mod date;

pub use date::DateData;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Temporal matching variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TemporalVariant {
    /// Date or date-range to match.
    Date(DateData),
}

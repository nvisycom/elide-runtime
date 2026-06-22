//! Conditions that gate when a policy rule applies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A condition that must be met for a policy rule to apply.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Condition {
    /// All specified document labels must be present.
    Labels {
        /// Document labels that must all be present.
        labels: Vec<String>,
    },
    /// A metadata key-value pair that must be present on the envelope.
    Metadata {
        /// Metadata key to match.
        key: String,
        /// Expected value. If `None`, any value for the key satisfies
        /// the condition.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
    },
}

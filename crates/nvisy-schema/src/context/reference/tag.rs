//! Tag reference data for keyword-based routing.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A keyword tag for classification and routing.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TagData {
    /// The tag value (e.g. `"pii"`, `"financial"`, `"hipaa"`).
    pub value: String,
    /// Optional category grouping related tags.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

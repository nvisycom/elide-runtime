//! Source node definition.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A data source that reads from an external provider.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SourceNode {
    /// Provider name used to resolve the connection (e.g. `"s3"`).
    pub provider: String,
    /// Stream name on the provider (e.g. `"read"`).
    pub stream: String,
}

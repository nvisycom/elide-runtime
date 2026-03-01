//! Target node definition.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A data sink that writes to an external provider.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TargetNode {
    /// Provider name used to resolve the connection (e.g. `"s3"`).
    pub provider: String,
    /// Stream name on the provider (e.g. `"write"`).
    pub stream: String,
}

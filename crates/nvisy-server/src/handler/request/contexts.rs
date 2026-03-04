//! Context request types.

use nvisy_ontology::context::Context;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

/// JSON request body for typed context upload.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextUpload {
    /// Optional actor identity. Defaults to a nil UUID when absent.
    #[serde(default)]
    pub actor_id: Option<Uuid>,
    /// The context to store.
    pub context: Context,
}

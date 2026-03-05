//! Context request types.

use nvisy_ontology::context::Context;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

/// JSON request body for typed context upload.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewContext {
    /// Actor identity that owns the context.
    pub actor_id: Uuid,
    /// The context to store.
    pub context: Context,
}

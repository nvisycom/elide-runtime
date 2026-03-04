//! Context request types.

use nvisy_ontology::context::Context;
use nvisy_registry::ActorId;
use schemars::JsonSchema;
use serde::Deserialize;

/// JSON request body for typed context upload.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextUpload {
    /// Actor identity that owns the context.
    pub actor_id: ActorId,
    /// The context to store.
    pub context: Context,
}

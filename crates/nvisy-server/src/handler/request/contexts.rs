//! Context request types.

use nvisy_ontology::context::Context;
use schemars::JsonSchema;
use serde::Deserialize;

/// Request body for `POST /contexts`: typed context upload.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewContext {
    /// The context to store.
    pub context: Context,
}

//! Context request types.

use nvisy_engine::pipeline::Context;
use schemars::JsonSchema;
use serde::Deserialize;

/// JSON request body for typed context upload.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewContext {
    /// The context to store.
    pub context: Context,
}

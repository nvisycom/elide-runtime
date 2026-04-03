//! Policy request types.

use nvisy_ontology::policy::Policy;
use schemars::JsonSchema;
use serde::Deserialize;

/// Request body for `POST /policies`: policy upload.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewPolicy {
    /// The policy to store.
    pub policy: Policy,
}

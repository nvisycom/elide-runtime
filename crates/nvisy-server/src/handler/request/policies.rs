//! Request bodies for `/policies` endpoints.

use nvisy_schema::policy::Policy;
use schemars::JsonSchema;
use serde::Deserialize;

/// Body for `POST /policies`. The full [`Policy`] inline — id +
/// version are caller-supplied (UUIDv7 + semver). Server stores
/// it under `(actor_id, policy.id, policy.version)`; re-posting
/// the same `(id, version)` returns `Conflict`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct NewPolicy(pub Policy);

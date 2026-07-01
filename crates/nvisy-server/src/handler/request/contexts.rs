//! Request bodies for `/contexts` endpoints.

use nvisy_schema::context::Context;
use schemars::JsonSchema;
use serde::Deserialize;

/// Body for `POST /contexts`. Same shape as `NewPolicy` —
/// caller-supplied id + version; the server keys storage by
/// `(actor_id, context.id, context.version)`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct NewContext(pub Context);

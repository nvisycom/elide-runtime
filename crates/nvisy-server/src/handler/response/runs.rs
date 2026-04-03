//! Run response types.

use nvisy_engine::pipeline::RunEntry;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::handler::request::Page;

/// Response body for `GET /runs`.
pub type RunList = Page<RunEntry>;

/// Response body for `POST /runs`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunId {
    /// Identifier assigned to the submitted pipeline run.
    pub id: Uuid,
}

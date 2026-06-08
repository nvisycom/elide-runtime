//! Request bodies for `/redactions` endpoints.

use nvisy_document::core::Plan;
use nvisy_document::phases::ingestion::ExportFile;
use nvisy_document::pipeline::{RedactionInput, RedactionOverride, RedactionStatus};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::handler::request::pagination::Pagination;

/// Body for `POST /redactions`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewRedaction {
    /// Detection pass this redaction targets.
    pub detection_id: Uuid,
    /// Per-entity decision overrides.
    #[serde(default)]
    pub overrides: Vec<RedactionOverride>,
    /// Per-phase behaviour knobs (validation thresholds etc.).
    #[serde(default)]
    pub plan: Plan,
    /// Sinks to write redacted content to.
    #[serde(default)]
    pub exports: Vec<ExportFile>,
}

impl NewRedaction {
    pub fn into_engine_input(self, actor_id: Uuid) -> RedactionInput {
        RedactionInput {
            actor_id,
            detection_id: self.detection_id,
            overrides: self.overrides,
            plan: self.plan,
            exports: self.exports,
        }
    }
}

/// Query parameters for `GET /redactions`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionQuery {
    /// Optional status filter.
    #[serde(default)]
    pub status: Option<RedactionStatus>,
    /// Optional filter: only redactions for this detection.
    #[serde(default)]
    pub detection_id: Option<Uuid>,
    /// Pagination knobs.
    #[serde(flatten)]
    pub pagination: Pagination,
}

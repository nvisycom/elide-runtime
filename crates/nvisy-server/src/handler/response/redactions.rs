//! Redaction response types.

use nvisy_document::pipeline::RedactionEntry;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use super::page::Page;

/// Response body for `GET /redactions`.
pub type RedactionList = Page<RedactionEntry>;

/// Response body for `POST /redactions`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionId {
    /// Identifier assigned to the submitted redaction pass.
    pub id: Uuid,
}

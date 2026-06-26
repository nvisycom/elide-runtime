//! Query parameters for `/files` endpoints. (Upload body is
//! raw bytes; descriptor fields come via HTTP headers
//! `Content-Type` + `Content-Disposition`.)

use schemars::JsonSchema;
use serde::Deserialize;

use super::pagination::Pagination;

/// Query parameters for `GET /files`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileQuery {
    /// Pagination knobs.
    #[serde(flatten)]
    pub pagination: Pagination,
}

//! Pagination query parameters for list endpoints.
//!
//! The matching response wrapper lives on [`Page`] in the response
//! module — call `Page::paginate(items, &pagination)` to apply the
//! query to a collection.
//!
//! [`Page`]: crate::handler::response::Page

use schemars::JsonSchema;
use serde::Deserialize;

/// Hard ceiling on `limit` — caps the page size a client can request
/// to bound memory use on large registries.
pub const MAX_PAGE_LIMIT: usize = 500;
/// Default page size when the caller doesn't specify `limit`.
pub const DEFAULT_PAGE_LIMIT: usize = 50;

/// Pagination parameters for list endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    /// Maximum number of items to return. Clamped to
    /// [`MAX_PAGE_LIMIT`] server-side (default: 50, max: 500).
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Number of items to skip (default: 0).
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    DEFAULT_PAGE_LIMIT
}

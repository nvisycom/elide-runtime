//! Paginated response wrapper.

use schemars::JsonSchema;
use serde::Serialize;

use crate::handler::request::Pagination;

/// Paginated response wrapper used by every list endpoint.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Page<T: Serialize + JsonSchema> {
    /// Total number of items before pagination.
    pub total: usize,
    /// Whether more items exist beyond this page.
    pub has_more: bool,
    /// The items in this page.
    pub items: Vec<T>,
}

impl<T: Serialize + JsonSchema> Page<T> {
    /// Apply pagination from a request-side [`Pagination`] query to
    /// a flat collection. `limit` is clamped to [`MAX_PAGE_LIMIT`]
    /// to bound memory use.
    ///
    /// [`MAX_PAGE_LIMIT`]: crate::handler::request::MAX_PAGE_LIMIT
    pub fn paginate(items: Vec<T>, pagination: &Pagination) -> Self {
        let limit = pagination
            .limit
            .min(crate::handler::request::MAX_PAGE_LIMIT);
        let total = items.len();
        let items: Vec<T> = items
            .into_iter()
            .skip(pagination.offset)
            .take(limit)
            .collect();
        Self {
            total,
            has_more: pagination.offset + items.len() < total,
            items,
        }
    }
}

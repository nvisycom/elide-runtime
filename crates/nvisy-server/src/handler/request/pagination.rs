//! Pagination query parameters and generic page response.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Pagination parameters for list endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    /// Maximum number of items to return (default: 50).
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Number of items to skip (default: 0).
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    50
}

impl Pagination {
    /// Apply pagination to a vector, returning a [`Page`].
    pub fn paginate<T: Serialize + JsonSchema>(&self, items: Vec<T>) -> Page<T> {
        let total = items.len();
        let items: Vec<T> = items
            .into_iter()
            .skip(self.offset)
            .take(self.limit)
            .collect();
        let has_more = self.offset + items.len() < total;
        Page {
            total,
            has_more,
            items,
        }
    }
}

/// Paginated response wrapper.
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

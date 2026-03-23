//! Pagination query parameters and generic page response.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Pagination parameters for list endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    /// Maximum number of items to return (default: 50).
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// Number of items to skip (default: 0).
    #[serde(default)]
    pub offset: u32,
}

fn default_limit() -> u32 {
    50
}

impl Pagination {
    /// Apply pagination to a vector, returning a [`Page`].
    pub fn paginate<T: Serialize + JsonSchema>(&self, items: Vec<T>) -> Page<T> {
        let total = items.len() as u32;
        let items: Vec<T> = items
            .into_iter()
            .skip(self.offset as usize)
            .take(self.limit as usize)
            .collect();
        let has_more = self.offset + items.len() as u32 > total;
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
    pub total: u32,
    /// Whether more items exist beyond this page.
    pub has_more: bool,
    /// The items in this page.
    pub items: Vec<T>,
}

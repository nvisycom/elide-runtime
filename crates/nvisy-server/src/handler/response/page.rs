//! Paginated response wrapper.

use nvisy_engine::registry::PagedResult;
use schemars::JsonSchema;
use serde::Serialize;

use crate::handler::request::Pagination;

/// Paginated response wrapper used by every list endpoint.
///
/// Two construction paths depending on where the data comes from:
///
/// - [`Page::paginate`] — for in-memory collections that have
///   already been materialised + sorted. Slices the full `Vec`.
/// - [`Page::from_paged`] — for registry-backed lists where the
///   storage layer did the windowing. Wraps a [`PagedResult`] +
///   computes `has_more` from `offset + items.len() < total`.
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
    /// Use [`Self::from_paged`] when the storage layer already did
    /// the windowing; this method is for in-memory collections that
    /// must be materialised in full (typically because they require
    /// sorting before slicing).
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

    /// Wrap a registry-side [`PagedResult`]. Maps each item through
    /// `f` (typically projecting a storage row to a wire summary)
    /// and computes `has_more` from the window position.
    pub fn from_paged<U>(
        paged: PagedResult<U>,
        pagination: &Pagination,
        f: impl FnMut(U) -> T,
    ) -> Self {
        let items: Vec<T> = paged.items.into_iter().map(f).collect();
        Self {
            has_more: pagination.offset + items.len() < paged.total,
            total: paged.total,
            items,
        }
    }
}

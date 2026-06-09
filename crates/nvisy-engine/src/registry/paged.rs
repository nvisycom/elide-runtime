//! [`PagedResult`]: registry pagination output.

/// A windowed slice of registry results plus the total count of
/// matching items.
///
/// `total` is the full per-actor count for the keyspace, computed
/// from a cheap key-only iteration; `items` is only the
/// `[offset, offset+limit)` window with its values deserialised.
///
/// The server layer wraps this into a `Page<T>` for the wire,
/// computing `has_more` from `offset + items.len() < total`.
#[derive(Debug, Clone)]
pub struct PagedResult<T> {
    /// The deserialised window slice.
    pub items: Vec<T>,
    /// Total number of items for this actor across the keyspace,
    /// regardless of the window.
    pub total: usize,
}

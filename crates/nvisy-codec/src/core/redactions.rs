//! [`Redactions`]: collection of `(location, redaction)` pairs handed
//! from the engine to a codec.
//!
//! The collection is a thin newtype over `Vec<(S, R)>`. Insertion is
//! an unconditional push — no overlap detection, no merging.
//!
//! # Why no codec-side dedup
//!
//! The engine layer's deduplication phase enforces that no two
//! surviving entities have overlapping locations (same-kind overlaps
//! fuse, cross-kind overlaps resolve via `ConflictResolution`). By the
//! time the redaction phase builds a batch, every entry's location is
//! disjoint from every other entry's.
//!
//! # Ordering
//!
//! The engine makes no ordering guarantee. [`IndexedHandle::redact`]
//! implementations reorder the batch as needed (right-to-left for text
//! or audio so earlier shifts don't invalidate later coordinates,
//! batched per page for PDF, …). Callers that need a single redaction
//! build a one-element batch with [`Redactions::single`].
//!
//! [`IndexedHandle::redact`]: crate::core::IndexedHandle::redact

use std::fmt;

/// A list of `(location, redaction)` pairs the engine hands to a
/// codec's `apply` entrypoint. Engine guarantees non-overlapping
/// locations; the codec is free to reorder for efficiency.
pub struct Redactions<S, R> {
    pub(crate) items: Vec<(S, R)>,
}

impl<S, R> Redactions<S, R> {
    /// Create an empty collection.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// One-element batch — the common "single redaction" convenience.
    pub fn single(location: S, redaction: R) -> Self {
        Self {
            items: vec![(location, redaction)],
        }
    }

    /// Total number of redactions queued.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the collection holds no redactions.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Append a `(location, redaction)` pair to the batch. No overlap
    /// check, no merge. Caller is responsible for non-overlap.
    pub fn push(&mut self, location: S, redaction: R) {
        self.items.push((location, redaction));
    }

    /// Consume the collection and return the underlying `(location,
    /// redaction)` pairs in insertion order.
    pub fn into_items(self) -> Vec<(S, R)> {
        self.items
    }
}

impl<S, R> Default for Redactions<S, R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, R> fmt::Debug for Redactions<S, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Redactions")
            .field("redactions", &self.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_appends_in_order() {
        let mut rs = Redactions::<(usize, usize), &'static str>::new();
        rs.push((0, 5), "a");
        rs.push((10, 15), "b");
        rs.push((20, 25), "c");
        let payloads: Vec<&'static str> = rs.items.iter().map(|(_, r)| *r).collect();
        assert_eq!(payloads, vec!["a", "b", "c"]);
    }

    #[test]
    fn len_and_is_empty_reflect_state() {
        let mut rs = Redactions::<usize, ()>::new();
        assert!(rs.is_empty());
        assert_eq!(rs.len(), 0);
        rs.push(0, ());
        assert!(!rs.is_empty());
        assert_eq!(rs.len(), 1);
    }
}

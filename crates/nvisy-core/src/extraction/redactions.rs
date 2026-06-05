//! [`Redactions<M>`]: collection of `(location, replacement)` pairs
//! handed from the operator side to the write-back side.
//!
//! The collection is a thin newtype over `Vec<(M::Location,
//! M::Replacement)>`. Insertion is an unconditional push — no overlap
//! detection, no merging.
//!
//! # Why no dedup
//!
//! Upstream phases (e.g. the document crate's deduplication step)
//! enforce that no two surviving entities have overlapping locations.
//! By the time a batch is built, every entry's location is disjoint
//! from every other entry's.
//!
//! # Ordering
//!
//! The producer makes no ordering guarantee.
//! [`RedactAt::redact_at`] implementations reorder the batch as needed
//! (right-to-left for text or audio so earlier shifts don't
//! invalidate later coordinates, batched per page for PDF, …).
//! Callers that need a single replacement build a one-element batch
//! with [`Redactions::single`].
//!
//! [`RedactAt::redact_at`]: super::RedactAt::redact_at

use std::fmt;

use crate::modality::ModalityData;
use crate::redaction::Redactable;

/// A list of `(location, replacement)` pairs handed to a
/// [`RedactAt<M>`] implementation. Producer guarantees non-overlapping
/// locations; the implementation is free to reorder for efficiency.
///
/// [`RedactAt<M>`]: super::RedactAt
pub struct Redactions<M: Redactable + ModalityData> {
    pub(crate) items: Vec<(M::Location, M::Replacement)>,
}

impl<M: Redactable + ModalityData> Redactions<M> {
    /// Create an empty collection.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// One-element batch — the common "single replacement" convenience.
    pub fn single(location: M::Location, replacement: M::Replacement) -> Self {
        Self {
            items: vec![(location, replacement)],
        }
    }

    /// Total number of replacements queued.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the collection holds no replacements.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Append a `(location, replacement)` pair to the batch. No
    /// overlap check, no merge. Caller is responsible for non-overlap.
    pub fn push(&mut self, location: M::Location, replacement: M::Replacement) {
        self.items.push((location, replacement));
    }

    /// Consume the collection and return the underlying
    /// `(location, replacement)` pairs in insertion order.
    pub fn into_items(self) -> Vec<(M::Location, M::Replacement)> {
        self.items
    }

    /// Borrow the underlying pairs in insertion order.
    pub fn items(&self) -> &[(M::Location, M::Replacement)] {
        &self.items
    }
}

impl<M: Redactable + ModalityData> Default for Redactions<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Redactable + ModalityData> fmt::Debug for Redactions<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Redactions")
            .field("len", &self.len())
            .finish()
    }
}

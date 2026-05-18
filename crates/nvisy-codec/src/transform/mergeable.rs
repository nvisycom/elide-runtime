//! [`Mergeable`] trait — overlap detection and merge semantics for
//! redaction payloads.

/// Trait for redactions that have a comparable extent within a span.
///
/// Required by [`Redactions`] to detect overlap on insert and to
/// produce a merged redaction under [`ConflictPolicy::Merge`].
///
/// [`Redactions`]: crate::transform::Redactions
/// [`ConflictPolicy::Merge`]: crate::transform::ConflictPolicy::Merge
pub trait Mergeable: Sized {
    /// Returns `true` when this redaction overlaps with `other`.
    ///
    /// Both redactions are assumed to live in the same span — callers
    /// must already group by span identity before calling this.
    fn overlaps(&self, other: &Self) -> bool;

    /// Try to combine two overlapping redactions into one.
    ///
    /// Returns `Some(merged)` when the redactions can be meaningfully
    /// combined (e.g. same replacement output, unioned extents).
    /// Returns `None` when they overlap but cannot be reconciled
    /// (e.g. different replacement strings, different image methods).
    fn try_merge(self, other: Self) -> Option<Self>;
}

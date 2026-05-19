//! Generic [`Redactions`] collection keyed by span identity.
//!
//! Stores redactions grouped by their target span, with overlap
//! detection on insert. The [`ConflictPolicy`] decides what happens
//! when two redactions overlap within the same span. See the
//! [`mergeable`] and [`policy`] modules for the traits and types
//! that govern collection behavior.
//!
//! Transforms consume a `Redactions` instead of a flat slice, so the
//! grouping + overlap-checking work is done once at the engine
//! boundary rather than re-done in each handler. The collection does
//! not expose raw map access: callers consume the collection via
//! [`IntoIterator`].
//!
//! [`Redactions`]: crate::transform::Redactions
//! [`ConflictPolicy`]: crate::transform::ConflictPolicy
//! [`mergeable`]: crate::transform::mergeable
//! [`policy`]: crate::transform::policy

use std::fmt;

use super::mergeable::Mergeable;
use super::policy::{ConflictPolicy, InsertError};

/// A set of redactions grouped by their target span, with overlap
/// detection on insert.
///
/// `S` is the span identity (e.g. [`TextLocation`], [`ImageLocation`])
/// and must implement [`PartialEq`] so the collection can find the
/// span an inserted redaction belongs to.
///
/// `R` is the per-span redaction payload (e.g. [`TextRedaction`])
/// and must implement [`Mergeable`] to support overlap detection.
///
/// Internally backed by a `Vec<(S, Vec<R>)>` — span counts are
/// typically small and insertion order matters for deterministic
/// downstream behavior, so a `HashMap` would be more cost than
/// benefit and would not work for keys with `f64` fields anyway.
///
/// [`TextLocation`]: nvisy_ontology::entity::TextLocation
/// [`ImageLocation`]: nvisy_ontology::entity::ImageLocation
/// [`TextRedaction`]: crate::transform::TextRedaction
pub struct Redactions<S, R> {
    policy: ConflictPolicy,
    spans: Vec<(S, Vec<R>)>,
}

impl<S, R> Redactions<S, R> {
    /// Create an empty collection with the given conflict policy.
    pub fn new(policy: ConflictPolicy) -> Self {
        Self {
            policy,
            spans: Vec::new(),
        }
    }

    /// The conflict policy in effect.
    pub fn policy(&self) -> ConflictPolicy {
        self.policy
    }

    /// Total number of redactions across all spans.
    pub fn len(&self) -> usize {
        self.spans.iter().map(|(_, rs)| rs.len()).sum()
    }

    /// Number of distinct spans that hold redactions.
    pub fn span_count(&self) -> usize {
        self.spans.len()
    }

    /// Returns `true` if the collection holds no redactions.
    pub fn is_empty(&self) -> bool {
        self.spans.iter().all(|(_, rs)| rs.is_empty())
    }
}

impl<S, R> Redactions<S, R>
where
    S: PartialEq,
    R: Mergeable,
{
    /// Insert a redaction targeting the given span.
    ///
    /// If the span already holds an overlapping redaction, behavior
    /// is determined by the configured [`ConflictPolicy`]:
    ///
    /// - [`Reject`]: returns [`InsertError::OverlapRejected`].
    /// - [`Merge`]: attempts to merge; returns
    ///   [`InsertError::NotMergeable`] when the merge fails.
    /// - [`Replace`]: drops the existing overlapping redaction and
    ///   inserts the new one.
    ///
    /// [`Reject`]: ConflictPolicy::Reject
    /// [`Merge`]: ConflictPolicy::Merge
    /// [`Replace`]: ConflictPolicy::Replace
    pub fn try_insert(&mut self, span: S, redaction: R) -> Result<(), InsertError> {
        let bucket = match self.spans.iter().position(|(s, _)| s == &span) {
            Some(idx) => &mut self.spans[idx].1,
            None => {
                self.spans.push((span, vec![redaction]));
                return Ok(());
            }
        };

        let overlap_idx = bucket.iter().position(|r| r.overlaps(&redaction));
        let Some(idx) = overlap_idx else {
            bucket.push(redaction);
            return Ok(());
        };

        match self.policy {
            ConflictPolicy::Reject => Err(InsertError::OverlapRejected),
            ConflictPolicy::Replace => {
                bucket[idx] = redaction;
                Ok(())
            }
            ConflictPolicy::Merge => {
                let existing = bucket.remove(idx);
                match existing.try_merge(redaction) {
                    Some(merged) => {
                        bucket.push(merged);
                        Ok(())
                    }
                    None => Err(InsertError::NotMergeable),
                }
            }
        }
    }
}

impl<S, R> Default for Redactions<S, R> {
    fn default() -> Self {
        Self::new(ConflictPolicy::default())
    }
}

impl<S, R> IntoIterator for Redactions<S, R> {
    type IntoIter = std::vec::IntoIter<(S, Vec<R>)>;
    type Item = (S, Vec<R>);

    /// Consume the collection, yielding each span paired with its
    /// owned redactions in insertion order.
    fn into_iter(self) -> Self::IntoIter {
        self.spans.into_iter()
    }
}

impl<S: fmt::Debug, R: fmt::Debug> fmt::Debug for Redactions<S, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Redactions")
            .field("policy", &self.policy)
            .field("spans", &self.spans.len())
            .field("redactions", &self.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct R {
        start: usize,
        end: usize,
        out: String,
    }

    impl R {
        fn new(start: usize, end: usize, out: &str) -> Self {
            Self {
                start,
                end,
                out: out.to_owned(),
            }
        }
    }

    impl Mergeable for R {
        fn overlaps(&self, other: &Self) -> bool {
            self.start < other.end && other.start < self.end
        }

        fn try_merge(self, other: Self) -> Option<Self> {
            if self.out != other.out {
                return None;
            }
            Some(R {
                start: self.start.min(other.start),
                end: self.end.max(other.end),
                out: self.out,
            })
        }
    }

    #[test]
    fn insert_into_new_span() {
        let mut rs = Redactions::<u32, R>::new(ConflictPolicy::Reject);
        rs.try_insert(0, R::new(0, 5, "x")).unwrap();
        assert_eq!(rs.len(), 1);
        assert_eq!(rs.span_count(), 1);
    }

    #[test]
    fn insert_non_overlapping_into_same_span() {
        let mut rs = Redactions::<u32, R>::new(ConflictPolicy::Reject);
        rs.try_insert(0, R::new(0, 5, "x")).unwrap();
        rs.try_insert(0, R::new(10, 15, "y")).unwrap();
        assert_eq!(rs.len(), 2);
        assert_eq!(rs.span_count(), 1);
    }

    #[test]
    fn insert_into_different_spans() {
        let mut rs = Redactions::<u32, R>::new(ConflictPolicy::Reject);
        rs.try_insert(0, R::new(0, 5, "x")).unwrap();
        rs.try_insert(1, R::new(0, 5, "x")).unwrap();
        assert_eq!(rs.span_count(), 2);
    }

    #[test]
    fn reject_policy_errors_on_overlap() {
        let mut rs = Redactions::<u32, R>::new(ConflictPolicy::Reject);
        rs.try_insert(0, R::new(0, 5, "x")).unwrap();
        let err = rs.try_insert(0, R::new(3, 8, "y")).unwrap_err();
        assert!(matches!(err, InsertError::OverlapRejected));
        assert_eq!(rs.len(), 1);
    }

    #[test]
    fn replace_policy_overwrites_overlap() {
        let mut rs = Redactions::<u32, R>::new(ConflictPolicy::Replace);
        rs.try_insert(0, R::new(0, 5, "x")).unwrap();
        rs.try_insert(0, R::new(3, 8, "y")).unwrap();
        assert_eq!(rs.len(), 1);
        let (_, items) = rs.into_iter().next().unwrap();
        assert_eq!(items[0].out, "y");
    }

    #[test]
    fn merge_policy_combines_same_output() {
        let mut rs = Redactions::<u32, R>::new(ConflictPolicy::Merge);
        rs.try_insert(0, R::new(0, 5, "x")).unwrap();
        rs.try_insert(0, R::new(3, 8, "x")).unwrap();
        assert_eq!(rs.len(), 1);
        let (_, items) = rs.into_iter().next().unwrap();
        assert_eq!(items[0].start, 0);
        assert_eq!(items[0].end, 8);
    }

    #[test]
    fn merge_policy_errors_when_unmergeable() {
        let mut rs = Redactions::<u32, R>::new(ConflictPolicy::Merge);
        rs.try_insert(0, R::new(0, 5, "x")).unwrap();
        let err = rs.try_insert(0, R::new(3, 8, "y")).unwrap_err();
        assert!(matches!(err, InsertError::NotMergeable));
    }

    #[test]
    fn into_iter_preserves_insertion_order() {
        let mut rs = Redactions::<u32, R>::new(ConflictPolicy::Reject);
        rs.try_insert(2, R::new(0, 5, "x")).unwrap();
        rs.try_insert(0, R::new(0, 5, "x")).unwrap();
        rs.try_insert(1, R::new(0, 5, "x")).unwrap();
        let spans: Vec<u32> = rs.into_iter().map(|(s, _)| s).collect();
        assert_eq!(spans, vec![2, 0, 1]);
    }

    #[test]
    fn empty_and_len() {
        let rs = Redactions::<u32, R>::new(ConflictPolicy::Reject);
        assert!(rs.is_empty());
        assert_eq!(rs.len(), 0);
    }
}

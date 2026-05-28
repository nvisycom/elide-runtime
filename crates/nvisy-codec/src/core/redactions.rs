//! Generic [`Redactions`] collection of `(location, redaction)` pairs
//! with overlap detection on insert.
//!
//! The collection is a flat `Vec<(S, R)>` ordered by insertion. On
//! [`try_insert`], `S::overlaps` checks for a collision with any
//! existing entry; under [`ConflictPolicy::Merge`], both
//! `S::try_merge` and `R::try_merge` must succeed to fuse the entries.
//!
//! Callers consume the collection via [`IntoIterator`] yielding `(S, R)`.
//!
//! [`Redactions`]: crate::core::Redactions
//! [`ConflictPolicy::Merge`]: crate::core::ConflictPolicy::Merge
//! [`try_insert`]: Redactions::try_insert

use std::fmt;

use derive_more::IntoIterator;
use nvisy_ontology::modality::{Mergeable, Overlap};

use super::policy::{ConflictPolicy, InsertError};

/// A set of `(location, redaction)` pairs with overlap detection on
/// insert.
///
/// `S` is the location key. It must implement [`Overlap`] (for
/// collision detection) and [`Mergeable`] (for the [`Merge`] policy).
///
/// `R` is the redaction payload. It must implement [`Mergeable`] —
/// the collection asks both `S` and `R` whether they can be merged
/// before fusing two colliding entries.
///
/// Internally backed by a `Vec<(S, R)>`. Entry counts are typically
/// small (per-document), so linear scans are cheap.
///
/// [`Merge`]: ConflictPolicy::Merge
#[derive(IntoIterator)]
pub struct Redactions<S, R> {
    policy: ConflictPolicy,
    #[into_iterator(owned)]
    items: Vec<(S, R)>,
}

impl<S, R> Redactions<S, R> {
    /// Create an empty collection with the given conflict policy.
    pub fn new(policy: ConflictPolicy) -> Self {
        Self {
            policy,
            items: Vec::new(),
        }
    }

    /// Total number of redactions.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the collection holds no redactions.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl<S, R> Redactions<S, R>
where
    S: Overlap + Mergeable,
    R: Mergeable,
{
    /// Insert a `(location, redaction)` pair.
    ///
    /// If `location` overlaps any existing entry's location, behavior
    /// is determined by the configured [`ConflictPolicy`]:
    ///
    /// - [`Reject`]: returns [`InsertError::RejectedOverlap`].
    /// - [`Merge`]: attempts to merge both location and redaction;
    ///   returns [`InsertError::NotMergeable`] if either rejects.
    /// - [`Replace`]: drops the existing overlapping entry and
    ///   inserts the new one.
    ///
    /// [`Reject`]: ConflictPolicy::Reject
    /// [`Merge`]: ConflictPolicy::Merge
    /// [`Replace`]: ConflictPolicy::Replace
    pub fn try_insert(&mut self, location: S, redaction: R) -> Result<(), InsertError> {
        let overlap_idx = self.items.iter().position(|(s, _)| s.overlaps(&location));
        let Some(idx) = overlap_idx else {
            self.items.push((location, redaction));
            return Ok(());
        };

        match self.policy {
            ConflictPolicy::Reject => Err(InsertError::RejectedOverlap),
            ConflictPolicy::Replace => {
                self.items[idx] = (location, redaction);
                Ok(())
            }
            ConflictPolicy::Merge => {
                let (existing_s, existing_r) = self.items.remove(idx);
                match (
                    existing_s.try_merge(location),
                    existing_r.try_merge(redaction),
                ) {
                    (Some(merged_s), Some(merged_r)) => {
                        self.items.push((merged_s, merged_r));
                        Ok(())
                    }
                    _ => Err(InsertError::NotMergeable),
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

impl<S, R> fmt::Debug for Redactions<S, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Redactions")
            .field("policy", &self.policy)
            .field("redactions", &self.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct S {
        start: usize,
        end: usize,
    }

    impl S {
        fn new(start: usize, end: usize) -> Self {
            Self { start, end }
        }
    }

    impl Overlap for S {
        fn overlaps(&self, other: &Self) -> bool {
            self.start < other.end && other.start < self.end
        }
    }

    impl Mergeable for S {
        fn try_merge(self, other: Self) -> Option<Self> {
            Some(Self {
                start: self.start.min(other.start),
                end: self.end.max(other.end),
            })
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct R(&'static str);

    impl Mergeable for R {
        fn try_merge(self, other: Self) -> Option<Self> {
            (self.0 == other.0).then_some(self)
        }
    }

    #[test]
    fn insert_non_overlapping_keeps_both() {
        let mut rs = Redactions::<S, R>::new(ConflictPolicy::Reject);
        rs.try_insert(S::new(0, 5), R("x")).unwrap();
        rs.try_insert(S::new(10, 15), R("y")).unwrap();
        assert_eq!(rs.len(), 2);
    }

    #[test]
    fn reject_policy_errors_on_overlap() {
        let mut rs = Redactions::<S, R>::new(ConflictPolicy::Reject);
        rs.try_insert(S::new(0, 5), R("x")).unwrap();
        let err = rs.try_insert(S::new(3, 8), R("y")).unwrap_err();
        assert!(matches!(err, InsertError::RejectedOverlap));
        assert_eq!(rs.len(), 1);
    }

    #[test]
    fn replace_policy_overwrites_overlap() {
        let mut rs = Redactions::<S, R>::new(ConflictPolicy::Replace);
        rs.try_insert(S::new(0, 5), R("x")).unwrap();
        rs.try_insert(S::new(3, 8), R("y")).unwrap();
        assert_eq!(rs.len(), 1);
        let (s, r) = rs.into_iter().next().unwrap();
        assert_eq!((s.start, s.end), (3, 8));
        assert_eq!(r.0, "y");
    }

    #[test]
    fn merge_policy_combines_same_payload() {
        let mut rs = Redactions::<S, R>::new(ConflictPolicy::Merge);
        rs.try_insert(S::new(0, 5), R("x")).unwrap();
        rs.try_insert(S::new(3, 8), R("x")).unwrap();
        assert_eq!(rs.len(), 1);
        let (s, _) = rs.into_iter().next().unwrap();
        assert_eq!((s.start, s.end), (0, 8));
    }

    #[test]
    fn merge_policy_errors_when_payload_differs() {
        let mut rs = Redactions::<S, R>::new(ConflictPolicy::Merge);
        rs.try_insert(S::new(0, 5), R("x")).unwrap();
        let err = rs.try_insert(S::new(3, 8), R("y")).unwrap_err();
        assert!(matches!(err, InsertError::NotMergeable));
    }

    #[test]
    fn into_iter_preserves_insertion_order() {
        let mut rs = Redactions::<S, R>::new(ConflictPolicy::Reject);
        rs.try_insert(S::new(20, 25), R("a")).unwrap();
        rs.try_insert(S::new(0, 5), R("b")).unwrap();
        rs.try_insert(S::new(10, 15), R("c")).unwrap();
        let starts: Vec<usize> = rs.into_iter().map(|(s, _)| s.start).collect();
        assert_eq!(starts, vec![20, 0, 10]);
    }
}

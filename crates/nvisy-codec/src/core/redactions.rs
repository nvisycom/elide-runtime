//! Generic [`Redactions`] collection of `(location, redaction)` pairs
//! with overlap-aware insert.
//!
//! The collection is a flat `Vec` of internal pairs, ordered by
//! insertion. On [`insert`], `S::overlaps` checks for a collision
//! with any existing entry; on collision the new pair is fused with
//! the existing one by merging the redaction payload first, then the
//! location. When the payload merge is rejected (e.g. two redactions
//! on the same span want different replacement outputs) both
//! originals are kept side by side — the collection never drops a
//! redaction.
//!
//! Callers consume the collection via [`IntoIterator`] yielding
//! `(S, R)` tuples.
//!
//! [`Redactions`]: crate::core::Redactions
//! [`insert`]: Redactions::insert

use std::fmt;

use nvisy_ontology::modality::{Mergeable, Overlap};

/// Crate-internal `(location, redaction)` bundle. Keeping the pair
/// as a named type lets [`Mergeable`] express the
/// "merge payload first, then location" rule once instead of
/// duplicating it on every collision branch in
/// [`Redactions::insert`]. Not exported from the crate — external
/// callers only see `Redactions::insert(location, redaction)`.
pub(crate) struct Pair<S, R> {
    pub(crate) location: S,
    pub(crate) redaction: R,
}

impl<S, R> Mergeable for Pair<S, R>
where
    S: Mergeable,
    R: Mergeable,
{
    fn try_merge(self, other: Self) -> Result<Self, (Self, Self)> {
        match self.redaction.try_merge(other.redaction) {
            Ok(merged_redaction) => {
                // Location identity is gated by the same fields on
                // both sides of `Overlap` and `Mergeable`, so a true
                // overlap always merges; if a future modality drifts
                // the two impls apart the `expect` surfaces it.
                let merged_location = self
                    .location
                    .try_merge(other.location)
                    .ok()
                    .expect("Overlap implies location Mergeable");
                Ok(Self {
                    location: merged_location,
                    redaction: merged_redaction,
                })
            }
            Err((existing_redaction, new_redaction)) => Err((
                Self {
                    location: self.location,
                    redaction: existing_redaction,
                },
                Self {
                    location: other.location,
                    redaction: new_redaction,
                },
            )),
        }
    }
}

/// A set of `(location, redaction)` pairs that fuses overlapping
/// entries on insert.
///
/// `S` must implement [`Overlap`] (for collision detection) and
/// [`Mergeable`] (for fusing location identity). `R` must implement
/// [`Mergeable`] for fusing redaction outputs. When either merge is
/// rejected, both pairs are kept side by side.
///
/// Internally backed by a `Vec`. Entry counts are typically small
/// (per-document), so linear scans are cheap.
pub struct Redactions<S, R> {
    pub(crate) items: Vec<Pair<S, R>>,
}

impl<S, R> Redactions<S, R> {
    /// Create an empty collection.
    pub fn new() -> Self {
        Self { items: Vec::new() }
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
    /// Insert a `(location, redaction)` pair, fusing it with any
    /// overlapping existing entry when both location and payload can
    /// be merged. When merging is rejected, both pairs are retained
    /// — the collection never drops a redaction.
    pub fn insert(&mut self, location: S, redaction: R) {
        let new_pair = Pair {
            location,
            redaction,
        };
        let Some(idx) = self
            .items
            .iter()
            .position(|pair| pair.location.overlaps(&new_pair.location))
        else {
            self.items.push(new_pair);
            return;
        };

        let existing_pair = self.items.remove(idx);
        match existing_pair.try_merge(new_pair) {
            Ok(merged_pair) => self.items.push(merged_pair),
            Err((existing_pair, new_pair)) => {
                self.items.push(existing_pair);
                self.items.push(new_pair);
            }
        }
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
        fn try_merge(self, other: Self) -> Result<Self, (Self, Self)> {
            Ok(Self {
                start: self.start.min(other.start),
                end: self.end.max(other.end),
            })
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct R(&'static str);

    impl Mergeable for R {
        fn try_merge(self, other: Self) -> Result<Self, (Self, Self)> {
            if self.0 == other.0 {
                Ok(self)
            } else {
                Err((self, other))
            }
        }
    }

    #[test]
    fn insert_non_overlapping_keeps_both() {
        let mut rs = Redactions::<S, R>::new();
        rs.insert(S::new(0, 5), R("x"));
        rs.insert(S::new(10, 15), R("y"));
        assert_eq!(rs.len(), 2);
    }

    #[test]
    fn overlap_with_same_payload_fuses() {
        let mut rs = Redactions::<S, R>::new();
        rs.insert(S::new(0, 5), R("x"));
        rs.insert(S::new(3, 8), R("x"));
        assert_eq!(rs.len(), 1);
        let pair = rs.items.into_iter().next().unwrap();
        assert_eq!((pair.location.start, pair.location.end), (0, 8));
    }

    #[test]
    fn overlap_with_different_payload_keeps_both() {
        let mut rs = Redactions::<S, R>::new();
        rs.insert(S::new(0, 5), R("x"));
        rs.insert(S::new(3, 8), R("y"));
        assert_eq!(rs.len(), 2);
    }

    #[test]
    fn iteration_preserves_insertion_order() {
        let mut rs = Redactions::<S, R>::new();
        rs.insert(S::new(20, 25), R("a"));
        rs.insert(S::new(0, 5), R("b"));
        rs.insert(S::new(10, 15), R("c"));
        let starts: Vec<usize> = rs.items.iter().map(|pair| pair.location.start).collect();
        assert_eq!(starts, vec![20, 0, 10]);
    }
}

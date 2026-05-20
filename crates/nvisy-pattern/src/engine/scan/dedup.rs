//! Overlap-aware deduplication of [`RawMatch`]es produced by the scan
//! phases.
//!
//! Same-kind matches that overlap are resolved to a single survivor:
//! higher confidence wins; ties break to the tighter span; full
//! duplicates collapse to the earlier-visited copy.

use std::cmp::Ordering;

use super::pattern_match::RawMatch;

/// Sort key for the dedup pass: `(start asc, confidence desc, span asc)`.
pub(in crate::engine) fn sort_for_dedup(raw: &mut [RawMatch]) {
    raw.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then(
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(Ordering::Equal),
            )
            .then_with(|| (a.end - a.start).cmp(&(b.end - b.start)))
    });
}

/// Drop any same-kind match that is "beaten" by another overlapping
/// match. Higher confidence wins; ties broken by tighter span; full
/// duplicates collapse to the earlier-visited copy.
pub(in crate::engine) fn dedup_overlapping(raw: &[RawMatch]) -> Vec<RawMatch> {
    raw.iter()
        .enumerate()
        .filter(|(i, m)| {
            !raw.iter().enumerate().any(|(j, other)| {
                i != &j
                    && other.entity_kind == m.entity_kind
                    && spans_overlap(other.start, other.end, m.start, m.end)
                    && beats(other, m, j < *i)
            })
        })
        .map(|(_, m)| m.clone())
        .collect()
}

pub(in crate::engine) fn spans_overlap(
    a_start: usize,
    a_end: usize,
    b_start: usize,
    b_end: usize,
) -> bool {
    a_start < b_end && b_start < a_end
}

/// Whether `winner` should knock `loser` out of the deduped result.
///
/// `winner_visited_first` is the tiebreaker for fully identical
/// `(start, end, confidence)` duplicates: only the earlier-visited copy
/// keeps its place.
pub(in crate::engine) fn beats(
    winner: &RawMatch,
    loser: &RawMatch,
    winner_visited_first: bool,
) -> bool {
    match winner
        .confidence
        .partial_cmp(&loser.confidence)
        .unwrap_or(Ordering::Equal)
    {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => {
            let winner_span = winner.end - winner.start;
            let loser_span = loser.end - loser.start;
            match winner_span.cmp(&loser_span) {
                Ordering::Less => true,
                Ordering::Greater => false,
                Ordering::Equal => winner_visited_first,
            }
        }
    }
}

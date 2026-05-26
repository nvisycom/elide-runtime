//! Temporal interval type with microsecond precision.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Microseconds per second.
const US_PER_SEC: i64 = 1_000_000;

/// A time interval within an audio stream, in microseconds.
///
/// Uses `i64` microseconds for sample-level precision without floating
/// point rounding. At 48kHz sample rate, one sample is ~20.8μs — well
/// within the 1μs resolution.
///
/// Use [`from_secs`] and [`start_secs`]
/// for ergonomic conversion to/from seconds.
///
/// [`from_secs`]: Self::from_secs
/// [`start_secs`]: Self::start_secs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct TimeSpan {
    /// Start time in microseconds from the beginning of the stream.
    pub start_us: i64,
    /// End time in microseconds from the beginning of the stream.
    pub end_us: i64,
}

impl TimeSpan {
    /// Create a time span from microsecond offsets.
    pub fn new(start_us: i64, end_us: i64) -> Self {
        Self { start_us, end_us }
    }

    /// Create a time span from seconds (converted to microseconds).
    pub fn from_secs(start: f64, end: f64) -> Self {
        Self {
            start_us: (start * US_PER_SEC as f64) as i64,
            end_us: (end * US_PER_SEC as f64) as i64,
        }
    }

    /// Start time in seconds.
    pub fn start_secs(&self) -> f64 {
        self.start_us as f64 / US_PER_SEC as f64
    }

    /// End time in seconds.
    pub fn end_secs(&self) -> f64 {
        self.end_us as f64 / US_PER_SEC as f64
    }

    /// Duration in microseconds.
    pub fn duration_us(&self) -> i64 {
        self.end_us - self.start_us
    }

    /// Duration in seconds.
    pub fn duration_secs(&self) -> f64 {
        self.duration_us() as f64 / US_PER_SEC as f64
    }

    /// Midpoint in microseconds.
    pub fn midpoint_us(&self) -> i64 {
        (self.start_us + self.end_us) / 2
    }

    /// Returns `true` if `t` (microseconds) falls within `[start, end)`.
    pub fn contains_us(&self, t: i64) -> bool {
        t >= self.start_us && t < self.end_us
    }

    /// Returns `true` if this span overlaps with `other`.
    pub fn overlaps(&self, other: &TimeSpan) -> bool {
        self.start_us < other.end_us && other.start_us < self.end_us
    }

    /// Returns the intersection of two spans, or `None` if they don't overlap.
    pub fn intersection(&self, other: &TimeSpan) -> Option<TimeSpan> {
        let start = self.start_us.max(other.start_us);
        let end = self.end_us.min(other.end_us);
        if start < end {
            Some(TimeSpan::new(start, end))
        } else {
            None
        }
    }

    /// Returns the smallest span that covers both `self` and `other`.
    pub fn union(&self, other: &TimeSpan) -> TimeSpan {
        TimeSpan::new(
            self.start_us.min(other.start_us),
            self.end_us.max(other.end_us),
        )
    }

    /// Convert this span to a `[start_sample, end_sample)` index range
    /// into a channel-interleaved sample buffer.
    ///
    /// Rounds half-up at the frame boundary, then multiplies by
    /// `channels` so the returned indices land on frame boundaries
    /// (no stereo channel swap on partial-frame edits).
    pub fn sample_range(&self, sample_rate: u32, channels: u16) -> (usize, usize) {
        let start_frame = us_to_frame(self.start_us, sample_rate);
        let end_frame = us_to_frame(self.end_us, sample_rate);
        let channels = channels as usize;
        (
            start_frame.saturating_mul(channels),
            end_frame.saturating_mul(channels),
        )
    }
}

/// Convert a microsecond offset to a frame index at `sample_rate`.
/// Half-up rounding keeps samples on either side of the boundary
/// consistently assigned.
fn us_to_frame(us: i64, sample_rate: u32) -> usize {
    if us <= 0 {
        return 0;
    }
    let num = (us as u128) * (sample_rate as u128) + 500_000;
    (num / 1_000_000) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_secs_roundtrip() {
        let span = TimeSpan::from_secs(1.5, 3.25);
        assert_eq!(span.start_us, 1_500_000);
        assert_eq!(span.end_us, 3_250_000);
        assert!((span.start_secs() - 1.5).abs() < 1e-9);
        assert!((span.end_secs() - 3.25).abs() < 1e-9);
    }

    #[test]
    fn duration() {
        let span = TimeSpan::new(1_000_000, 3_500_000);
        assert_eq!(span.duration_us(), 2_500_000);
        assert!((span.duration_secs() - 2.5).abs() < 1e-9);
    }

    #[test]
    fn contains() {
        let span = TimeSpan::new(1_000_000, 5_000_000);
        assert!(span.contains_us(1_000_000));
        assert!(span.contains_us(3_000_000));
        assert!(!span.contains_us(5_000_000)); // exclusive end
        assert!(!span.contains_us(500_000));
    }

    #[test]
    fn overlaps() {
        let a = TimeSpan::new(1_000_000, 3_000_000);
        let b = TimeSpan::new(2_000_000, 4_000_000);
        let c = TimeSpan::new(3_000_000, 5_000_000);
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c)); // touching at boundary = no overlap
    }

    #[test]
    fn intersection() {
        let a = TimeSpan::new(1_000_000, 4_000_000);
        let b = TimeSpan::new(2_000_000, 5_000_000);
        let i = a.intersection(&b).unwrap();
        assert_eq!(i.start_us, 2_000_000);
        assert_eq!(i.end_us, 4_000_000);

        let c = TimeSpan::new(4_000_000, 6_000_000);
        assert!(a.intersection(&c).is_none());
    }

    #[test]
    fn union() {
        let a = TimeSpan::new(1_000_000, 3_000_000);
        let b = TimeSpan::new(2_000_000, 5_000_000);
        let u = a.union(&b);
        assert_eq!(u.start_us, 1_000_000);
        assert_eq!(u.end_us, 5_000_000);
    }

    #[test]
    fn sample_range_mono_aligns_to_frames() {
        let span = TimeSpan::new(3_000, 6_000);
        let (start, end) = span.sample_range(1000, 1);
        assert_eq!((start, end), (3, 6));
    }

    #[test]
    fn sample_range_stereo_aligns_to_frames() {
        let span = TimeSpan::new(3_000, 6_000);
        let (start, end) = span.sample_range(1000, 2);
        assert_eq!((start, end), (6, 12));
    }

    #[test]
    fn sample_range_clamps_negative_to_zero() {
        let span = TimeSpan::new(-1_000, 2_000);
        let (start, end) = span.sample_range(1000, 1);
        assert_eq!((start, end), (0, 2));
    }

    #[test]
    fn sample_level_precision() {
        // At 48kHz, one sample = 20.833... μs
        // Two adjacent samples should be distinguishable
        let sample_duration_us = 1_000_000 / 48_000; // 20μs (truncated)
        let a = TimeSpan::new(0, sample_duration_us);
        let b = TimeSpan::new(sample_duration_us, sample_duration_us * 2);
        assert!(!a.overlaps(&b));
    }
}

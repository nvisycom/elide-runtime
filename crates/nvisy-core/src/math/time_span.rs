//! Temporal interval type.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A time interval within an audio stream.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TimeSpan {
    /// Start time in seconds from the beginning of the stream.
    pub start_secs: f64,
    /// End time in seconds from the beginning of the stream.
    pub end_secs: f64,
}

impl TimeSpan {
    /// Create a new time span.
    pub fn new(start_secs: f64, end_secs: f64) -> Self {
        Self {
            start_secs,
            end_secs,
        }
    }

    /// Duration of the span in seconds.
    pub fn duration(&self) -> f64 {
        self.end_secs - self.start_secs
    }

    /// Midpoint of the span in seconds.
    pub fn midpoint(&self) -> f64 {
        (self.start_secs + self.end_secs) / 2.0
    }

    /// Returns `true` if `t` falls within `[start, end)`.
    pub fn contains(&self, t: f64) -> bool {
        t >= self.start_secs && t < self.end_secs
    }

    /// Returns `true` if this span overlaps with `other`.
    pub fn overlaps(&self, other: &TimeSpan) -> bool {
        self.start_secs < other.end_secs && other.start_secs < self.end_secs
    }

    /// Returns the intersection of two spans, or `None` if they don't overlap.
    pub fn intersection(&self, other: &TimeSpan) -> Option<TimeSpan> {
        let start = self.start_secs.max(other.start_secs);
        let end = self.end_secs.min(other.end_secs);
        if start < end {
            Some(TimeSpan::new(start, end))
        } else {
            None
        }
    }

    /// Returns the smallest span that covers both `self` and `other`.
    pub fn union(&self, other: &TimeSpan) -> TimeSpan {
        TimeSpan::new(
            self.start_secs.min(other.start_secs),
            self.end_secs.max(other.end_secs),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration() {
        let span = TimeSpan::new(1.0, 3.5);
        assert!((span.duration() - 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn midpoint() {
        let span = TimeSpan::new(2.0, 4.0);
        assert!((span.midpoint() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn contains() {
        let span = TimeSpan::new(1.0, 5.0);
        assert!(span.contains(1.0));
        assert!(span.contains(3.0));
        assert!(!span.contains(5.0)); // exclusive end
        assert!(!span.contains(0.5));
    }

    #[test]
    fn overlaps() {
        let a = TimeSpan::new(1.0, 3.0);
        let b = TimeSpan::new(2.0, 4.0);
        let c = TimeSpan::new(3.0, 5.0);
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c)); // touching at boundary = no overlap
    }

    #[test]
    fn intersection() {
        let a = TimeSpan::new(1.0, 4.0);
        let b = TimeSpan::new(2.0, 5.0);
        let i = a.intersection(&b).unwrap();
        assert!((i.start_secs - 2.0).abs() < f64::EPSILON);
        assert!((i.end_secs - 4.0).abs() < f64::EPSILON);

        let c = TimeSpan::new(4.0, 6.0);
        assert!(a.intersection(&c).is_none());
    }

    #[test]
    fn union() {
        let a = TimeSpan::new(1.0, 3.0);
        let b = TimeSpan::new(2.0, 5.0);
        let u = a.union(&b);
        assert!((u.start_secs - 1.0).abs() < f64::EPSILON);
        assert!((u.end_secs - 5.0).abs() < f64::EPSILON);
    }
}

//! [`Confidence`] — a validated `[0.0, 1.0]` confidence score.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A confidence score in the closed range `[0.0, 1.0]`.
///
/// Construct with [`Confidence::new`]; the constructor returns
/// [`None`] for values outside the valid range or non-finite floats
/// (`NaN`, `±∞`). Read the inner score with [`Confidence::get`].
/// The API mirrors [`std::num::NonZero::new`] /
/// [`std::num::NonZero::get`].
///
/// `Confidence` is `Copy` and cheap to pass by value. Operations
/// that combine confidences (averaging, multiplying) are not
/// provided directly — callers should compute on `f64` via `.get()`
/// and re-construct.
///
/// # Examples
///
/// ```
/// use nvisy_ontology::primitive::Confidence;
///
/// let c = Confidence::new(0.85).unwrap();
/// assert_eq!(c.get(), 0.85);
///
/// assert!(Confidence::new(1.5).is_none());
/// assert!(Confidence::new(f64::NAN).is_none());
/// ```
///
/// [`Confidence::new`]: Self::new
/// [`Confidence::get`]: Self::get
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Confidence(f64);

impl Confidence {
    /// Construct a [`Confidence`] from a raw score, returning
    /// [`None`] when `value` is `NaN`, `±∞`, or outside the closed
    /// range `[0.0, 1.0]`.
    pub fn new(value: f64) -> Option<Self> {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Returns the inner score.
    pub fn get(&self) -> f64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_in_range_values() {
        assert_eq!(Confidence::new(0.0).unwrap().get(), 0.0);
        assert_eq!(Confidence::new(1.0).unwrap().get(), 1.0);
        assert_eq!(Confidence::new(0.5).unwrap().get(), 0.5);
    }

    #[test]
    fn rejects_out_of_range_values() {
        assert!(Confidence::new(-0.1).is_none());
        assert!(Confidence::new(1.1).is_none());
    }

    #[test]
    fn rejects_non_finite_values() {
        assert!(Confidence::new(f64::NAN).is_none());
        assert!(Confidence::new(f64::INFINITY).is_none());
        assert!(Confidence::new(f64::NEG_INFINITY).is_none());
    }

    #[test]
    fn serde_round_trips() {
        let c = Confidence::new(0.42).unwrap();
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, "0.42");
        let back: Confidence = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
    }
}

//! [`Confidence`] — a validated `[0.0, 1.0]` confidence score.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

/// A confidence score in the closed range `[0.0, 1.0]`.
///
/// Construct with [`Confidence::new`]; the constructor returns
/// [`None`] for values outside the valid range or non-finite floats
/// (`NaN`, `±∞`). Read the inner score with [`Confidence::get`].
/// The API mirrors [`NonZero::new`] / [`NonZero::get`].
///
/// `Confidence` is `Copy` and cheap to pass by value. Operations
/// that combine confidences (averaging, multiplying) are not
/// provided directly — callers should compute on `f64` via `.get()`
/// and re-construct.
///
/// Deserialization is strict: an input outside `[0.0, 1.0]` or any
/// non-finite float fails with an error. The newtype is a real
/// invariant, not a hint.
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
/// [`NonZero::new`]: std::num::NonZero::new
/// [`NonZero::get`]: std::num::NonZero::get
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct Confidence(f64);

impl<'de> Deserialize<'de> for Confidence {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = f64::deserialize(deserializer)?;
        Self::new(value).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "confidence {value} out of range [0.0, 1.0] or non-finite"
            ))
        })
    }
}

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

    /// Construct a [`Confidence`] by clamping a raw score into
    /// `[0.0, 1.0]`. Use when the input is *known to be in range up
    /// to float rounding* — e.g. a softmax mean that might come out
    /// as `1.0000000000000002`. Non-finite inputs (`NaN`, `±∞`) map
    /// to `0.0`: their presence almost always indicates an upstream
    /// bug, so [`debug_assert!`] catches it during development while
    /// release builds degrade safely.
    pub fn clamped(value: f64) -> Self {
        debug_assert!(value.is_finite(), "non-finite confidence: {value}");
        if !value.is_finite() {
            return Self(0.0);
        }
        Self(value.clamp(0.0, 1.0))
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

    #[test]
    fn deserialize_rejects_out_of_range() {
        assert!(serde_json::from_str::<Confidence>("1.4").is_err());
        assert!(serde_json::from_str::<Confidence>("-0.2").is_err());
    }

    #[test]
    fn deserialize_rejects_non_finite() {
        // NaN and Infinity aren't representable in standard JSON;
        // serde_json reports them as invalid number syntax. We
        // still cover the constructor path via the new() check.
        assert!(serde_json::from_str::<Confidence>("null").is_err());
    }
}

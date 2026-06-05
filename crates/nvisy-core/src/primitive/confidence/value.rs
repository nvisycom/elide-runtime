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
/// use nvisy_core::primitive::Confidence;
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
    /// Upper bound of the valid range. Equivalent to
    /// `Confidence::new(1.0).unwrap()`, but `const` so it works in
    /// constant contexts. Mirrors the `MIN` / `MAX` constants on
    /// primitive integer types (`u32::MAX`, `i64::MAX`, …).
    pub const MAX: Self = Self(1.0);
    /// Lower bound of the valid range. Equivalent to
    /// `Confidence::new(0.0).unwrap()`, but `const` so it works in
    /// constant contexts. Mirrors the `MIN` / `MAX` constants on
    /// primitive integer types (`u32::MIN`, `i64::MIN`, …).
    pub const MIN: Self = Self(0.0);

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
    /// `[0.0, 1.0]`. Use when the input is *known to be in range
    /// modulo float rounding* — a softmax mean that might come out
    /// as `1.0000000000000002`, a calibration offset that nudges a
    /// 0.99 slightly above 1.0, etc.
    ///
    /// **Panics** on `NaN` or `±∞`. Non-finite values cannot have
    /// come from a well-behaved producer; loud failure beats the
    /// previous silent-degrade-to-`0.0` behaviour, which masked
    /// upstream bugs by quietly dropping detections.
    ///
    /// For floats from external / untrusted sources (network
    /// payloads, model server responses, user input), use
    /// [`Self::try_clamped`] and decide what to do with the rejection
    /// at the boundary.
    pub fn clamped(value: f64) -> Self {
        assert!(
            value.is_finite(),
            "non-finite confidence: {value} (NaN/±∞ indicates an upstream bug)"
        );
        Self(value.clamp(0.0, 1.0))
    }

    /// Fallible counterpart to [`Self::clamped`] for floats whose
    /// finiteness isn't pre-validated. Returns [`None`] on `NaN` /
    /// `±∞`; otherwise clamps into `[0.0, 1.0]`. Use at the boundary
    /// where untrusted floats enter the pipeline (model server
    /// responses, deserialised user input) and the caller knows
    /// what fallback to apply on rejection.
    pub fn try_clamped(value: f64) -> Option<Self> {
        value.is_finite().then(|| Self(value.clamp(0.0, 1.0)))
    }

    /// Returns the inner score.
    pub fn get(&self) -> f64 {
        self.0
    }

    /// Add `delta` to the score, saturating at `1.0`. Negative deltas
    /// saturate at `0.0`. Non-finite deltas (`NaN`, `±∞`) are treated
    /// as `0.0` — same posture as [`Self::clamped`].
    ///
    /// Distinct from combining two confidences (which the type
    /// deliberately doesn't expose): this is a scalar adjustment to
    /// a single score, used by context-aware boosting / penalty.
    pub fn saturating_add(self, delta: f64) -> Self {
        debug_assert!(delta.is_finite(), "non-finite confidence delta: {delta}");
        let delta = if delta.is_finite() { delta } else { 0.0 };
        Self((self.0 + delta).clamp(0.0, 1.0))
    }

    /// Subtract `delta` from the score, saturating at `0.0`. Negative
    /// deltas saturate at `1.0`. Symmetric to [`Self::saturating_add`].
    pub fn saturating_sub(self, delta: f64) -> Self {
        self.saturating_add(-delta)
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
    fn min_max_constants_equal_boundary_values() {
        assert_eq!(Confidence::MIN.get(), 0.0);
        assert_eq!(Confidence::MAX.get(), 1.0);
        assert_eq!(Confidence::MIN, Confidence::new(0.0).unwrap());
        assert_eq!(Confidence::MAX, Confidence::new(1.0).unwrap());
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
    fn clamped_handles_rounding_above_one() {
        assert_eq!(Confidence::clamped(1.0000000000000002).get(), 1.0);
        assert_eq!(Confidence::clamped(-0.0).get(), 0.0);
    }

    #[test]
    #[should_panic(expected = "non-finite confidence")]
    fn clamped_panics_on_nan() {
        let _ = Confidence::clamped(f64::NAN);
    }

    #[test]
    #[should_panic(expected = "non-finite confidence")]
    fn clamped_panics_on_infinity() {
        let _ = Confidence::clamped(f64::INFINITY);
    }

    #[test]
    fn try_clamped_accepts_finite() {
        assert_eq!(Confidence::try_clamped(0.5).unwrap().get(), 0.5);
        assert_eq!(Confidence::try_clamped(2.0).unwrap().get(), 1.0);
        assert_eq!(Confidence::try_clamped(-1.0).unwrap().get(), 0.0);
    }

    #[test]
    fn try_clamped_rejects_non_finite() {
        assert!(Confidence::try_clamped(f64::NAN).is_none());
        assert!(Confidence::try_clamped(f64::INFINITY).is_none());
        assert!(Confidence::try_clamped(f64::NEG_INFINITY).is_none());
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

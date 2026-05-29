//! [`ConfidenceThreshold`] — a validated lower bound for
//! [`Confidence`] values.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use super::Confidence;

/// A lower-bound threshold on [`Confidence`]. Wraps a [`Confidence`]
/// so it inherits the `[0.0, 1.0]` + finite-float invariant, and
/// exposes [`admits`] as the single named operation for the "is this
/// entity above the cutoff?" question.
///
/// The shape exists so producers can't reach for a bare `f64` that
/// silently accepts `1.5` or `NaN`, and so consumers don't open-code
/// `entity.confidence.get() >= threshold.get()` everywhere with the
/// inequality direction free to drift. The contract is fixed: the
/// threshold *admits* a confidence at or above its value (inclusive
/// on the boundary).
///
/// Wire shape: `ConfidenceThreshold` serialises as a bare number,
/// same as [`Confidence`]. Existing `"confidenceThreshold": 0.85`
/// payloads round-trip without change.
///
/// # Examples
///
/// ```
/// use nvisy_ontology::primitive::{Confidence, ConfidenceThreshold};
///
/// let cutoff = ConfidenceThreshold::new(0.85).unwrap();
/// assert!(cutoff.admits(Confidence::new(0.90).unwrap()));
/// assert!(cutoff.admits(Confidence::new(0.85).unwrap())); // inclusive
/// assert!(!cutoff.admits(Confidence::new(0.84).unwrap()));
/// ```
///
/// [`admits`]: Self::admits
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct ConfidenceThreshold(Confidence);

impl<'de> Deserialize<'de> for ConfidenceThreshold {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Confidence::deserialize(deserializer).map(Self)
    }
}

impl ConfidenceThreshold {
    /// Construct a [`ConfidenceThreshold`] from a raw score,
    /// returning [`None`] when the value is outside `[0.0, 1.0]` or
    /// non-finite. Mirrors [`Confidence::new`] — the threshold's
    /// validity domain is exactly the same as a confidence score.
    pub fn new(value: f64) -> Option<Self> {
        Confidence::new(value).map(Self)
    }

    /// Construct a [`ConfidenceThreshold`] by clamping a raw score
    /// into `[0.0, 1.0]`, panicking on `NaN` / `±∞`. Mirrors
    /// [`Confidence::clamped`] — use when the value is a literal /
    /// internal constant known to be in range modulo float rounding.
    /// For floats from external sources, prefer
    /// [`Self::new`] and decide what to do with the rejection at
    /// the boundary.
    pub fn clamped(value: f64) -> Self {
        Self(Confidence::clamped(value))
    }

    /// Wrap an already-validated [`Confidence`] as a threshold.
    pub const fn from_confidence(value: Confidence) -> Self {
        Self(value)
    }

    /// Returns the inner threshold as a [`Confidence`].
    pub fn confidence(&self) -> Confidence {
        self.0
    }

    /// Returns the inner score as a raw `f64`. Prefer [`Self::admits`]
    /// at comparison sites — this accessor exists for serialisation,
    /// telemetry, and prompt construction where the raw number is
    /// what's needed.
    pub fn get(&self) -> f64 {
        self.0.get()
    }

    /// Returns `true` when `confidence` is **at or above** the
    /// threshold. The boundary is inclusive: a threshold of `0.85`
    /// admits a confidence of exactly `0.85`.
    pub fn admits(self, confidence: Confidence) -> bool {
        confidence.get() >= self.0.get()
    }
}

impl From<Confidence> for ConfidenceThreshold {
    fn from(c: Confidence) -> Self {
        Self::from_confidence(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admits_above_below_and_exact() {
        let cutoff = ConfidenceThreshold::new(0.5).unwrap();
        assert!(cutoff.admits(Confidence::new(0.6).unwrap()));
        assert!(cutoff.admits(Confidence::new(0.5).unwrap()));
        assert!(!cutoff.admits(Confidence::new(0.49).unwrap()));
    }

    #[test]
    fn boundaries() {
        let zero = ConfidenceThreshold::new(0.0).unwrap();
        assert!(zero.admits(Confidence::new(0.0).unwrap()));
        let one = ConfidenceThreshold::new(1.0).unwrap();
        assert!(one.admits(Confidence::new(1.0).unwrap()));
        assert!(!one.admits(Confidence::new(0.99).unwrap()));
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(ConfidenceThreshold::new(-0.1).is_none());
        assert!(ConfidenceThreshold::new(1.1).is_none());
        assert!(ConfidenceThreshold::new(f64::NAN).is_none());
        assert!(ConfidenceThreshold::new(f64::INFINITY).is_none());
    }

    #[test]
    fn clamped_clamps_into_range() {
        assert_eq!(ConfidenceThreshold::clamped(0.5).get(), 0.5);
        assert_eq!(ConfidenceThreshold::clamped(1.5).get(), 1.0);
        assert_eq!(ConfidenceThreshold::clamped(-0.5).get(), 0.0);
    }

    #[test]
    #[should_panic(expected = "non-finite confidence")]
    fn clamped_panics_on_nan() {
        let _ = ConfidenceThreshold::clamped(f64::NAN);
    }

    #[test]
    fn serialises_as_bare_number() {
        let cutoff = ConfidenceThreshold::new(0.85).unwrap();
        assert_eq!(serde_json::to_string(&cutoff).unwrap(), "0.85");
        let parsed: ConfidenceThreshold = serde_json::from_str("0.85").unwrap();
        assert_eq!(parsed, cutoff);
    }

    #[test]
    fn deserialize_rejects_out_of_range() {
        assert!(serde_json::from_str::<ConfidenceThreshold>("1.5").is_err());
        assert!(serde_json::from_str::<ConfidenceThreshold>("-0.1").is_err());
    }
}

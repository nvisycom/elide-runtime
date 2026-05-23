//! Language detection: public [`LanguagePolicy`] factory trait,
//! built-in implementations, and supporting types.
//!
//! Two layers:
//!
//! - **`LanguagePolicy`** is the abstraction the [`NlpEngine`] plugs
//!   into. A policy is a factory: per call it builds a fresh detector
//!   restricted to a caller-supplied language set (or to every
//!   language the policy can produce when the caller has no
//!   preference).
//! - **`LanguageDetector`** is what a policy produces. It exposes a
//!   single `detect` method. Language scope is baked into the detector
//!   at construction time — the engine never asks a detector to
//!   re-narrow.
//!
//! Built-in: [`LinguaLanguagePolicy`] wraps the [`lingua`] crate.
//!
//! [`NlpEngine`]: crate::engine::NlpEngine
//! [`lingua`]: https://crates.io/crates/lingua

mod detection;
mod dyn_policy;
mod lingua;

use nvisy_ontology::primitive::LanguageTag;

pub use self::detection::{LanguageDetection, LanguageProvenance, LanguageSpan};
pub(crate) use self::dyn_policy::DynLanguagePolicy;
pub use self::lingua::{LinguaLanguageDetector, LinguaLanguagePolicy};
use crate::error::Result;

/// A produced detector that recognises language(s) within a text
/// string.
///
/// Implementations bake their language scope in at construction
/// time — the engine never asks a detector to re-narrow. To detect
/// against a different language set, ask the [`LanguagePolicy`] for
/// a fresh detector via [`LanguagePolicy::detector_for`] or
/// [`LanguagePolicy::detector_for_all`].
///
/// Third-party policies need this trait public so their `Detector`
/// associated type can satisfy the bound on [`LanguagePolicy`].
pub trait LanguageDetector: Send + Sync {
    /// Detect languages in `text`.
    ///
    /// Single-language detectors return a one-element vec; backends
    /// with mixed-language support return one entry per detected
    /// region with [`LanguageDetection::span`] populated. An empty
    /// vec means "couldn't decide."
    fn detect(&self, text: &str) -> Result<Vec<LanguageDetection>>;
}

/// Factory for building a language detector keyed on a per-call
/// language set.
///
/// The engine holds a policy and asks for a fresh detector each
/// call. Two entrypoints:
///
/// - [`detector_for_all`] — "I don't know what language to expect;
///   consider everything you can."
/// - [`detector_for`] — "Only consider these languages."
///
/// Implementations may cache produced detectors internally;
/// callers must not assume a fresh allocation per call.
///
/// [`detector_for_all`]: Self::detector_for_all
/// [`detector_for`]: Self::detector_for
pub trait LanguagePolicy: Send + Sync {
    /// Concrete detector type this policy produces. Surfaced as an
    /// associated type so impls advertise their detector kind and
    /// callers that go through the policy directly (rather than via
    /// the engine) get a typed handle.
    type Detector: LanguageDetector + 'static;

    /// Build a detector that considers every language the policy
    /// can produce. Used when the caller hasn't restricted the set.
    fn detector_for_all(&self) -> Self::Detector;

    /// Build a detector restricted to `languages`.
    ///
    /// Implementations decide how to handle an empty slice and how
    /// to handle tags they don't recognise — the convention in this
    /// crate is to fall back to [`detector_for_all`] for empty
    /// input and silently skip unmappable tags.
    ///
    /// [`detector_for_all`]: Self::detector_for_all
    fn detector_for(&self, languages: &[LanguageTag]) -> Self::Detector;
}

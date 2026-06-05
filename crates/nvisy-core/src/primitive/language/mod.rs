//! Language-related primitives.
//!
//! BCP-47 [`LanguageTag`] plus [`LanguageDetection`] — the result
//! shape every language-detection backend returns, with provenance
//! and an optional per-region byte-offset span — plus
//! [`LanguageDetections`], the typed-map artifact key that bundles
//! the per-document detection set.

mod detection;
mod detections;
mod tag;

pub use self::detection::{LanguageDetection, LanguageProvenance, LanguageSpan};
pub use self::detections::LanguageDetections;
pub use self::tag::LanguageTag;

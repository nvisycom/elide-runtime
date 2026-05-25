//! Language-related primitives.
//!
//! BCP-47 [`LanguageTag`] plus [`LanguageDetection`] — the result
//! shape every language-detection backend returns, with provenance
//! and an optional per-region byte-offset span.

mod detection;
mod tag;

pub use self::detection::{LanguageDetection, LanguageProvenance, LanguageSpan};
pub use self::tag::LanguageTag;

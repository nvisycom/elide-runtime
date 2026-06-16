//! Primitive types used across the ontology.
//!
//! Submodules group related primitives:
//! - `confidence` — [`Confidence`] scores + [`ConfidenceThreshold`]
//!   cutoffs.
//! - `geometry` — 2D coordinate types ([`BoundingBox`], [`Polygon`]).
//! - `language` — language tags + per-region detection results
//!   ([`LanguageTag`], [`LanguageDetection`]).
//! - `rendering` — output-side knobs ([`Color`], [`Dpi`]).
//!
//! Root-level primitives: [`CountryCode`] (ISO 3166-1 alpha-2),
//! [`TimeSpan`] (temporal intervals).

mod confidence;
mod country;
mod geometry;
mod language;
mod rendering;
mod time_span;

pub use self::confidence::{Confidence, ConfidenceThreshold};
pub use self::country::CountryCode;
pub use self::geometry::{
    BoundingBox, Dimensions, IBoundingBox, NormalizedBoundingBox, Polygon, Vertex,
};
pub use self::language::{
    LanguageDetection, LanguageDetections, LanguageProvenance, LanguageSpan, LanguageTag,
};
pub use self::rendering::{Color, Dpi};
pub use self::time_span::TimeSpan;

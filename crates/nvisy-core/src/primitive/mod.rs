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
//! [`TimeSpan`] is the single root-level primitive — temporal
//! intervals don't have any companion types worth grouping yet.

mod confidence;
mod geometry;
mod language;
mod rendering;
mod time_span;

pub use self::confidence::{Confidence, ConfidenceThreshold};
pub use self::geometry::{
    BoundingBox, Dimensions, IBoundingBox, NormalizedBoundingBox, Polygon, Vertex,
};
pub use self::language::{
    LanguageDetection, LanguageDetections, LanguageProvenance, LanguageSpan, LanguageTag,
};
pub use self::rendering::{Color, Dpi};
pub use self::time_span::TimeSpan;

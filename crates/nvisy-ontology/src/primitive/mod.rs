//! Primitive types used across the ontology.
//!
//! Spatial primitives (bounding boxes, polygons), temporal intervals,
//! language tags, and rendering types.

mod bounding_box;
mod color;
mod confidence;
mod confidence_threshold;
mod dpi;
mod language;
mod polygon;
mod time_span;

pub use self::bounding_box::{BoundingBox, IBoundingBox};
pub use self::color::Color;
pub use self::confidence::Confidence;
pub use self::confidence_threshold::ConfidenceThreshold;
pub use self::dpi::Dpi;
pub use self::language::{LanguageDetection, LanguageProvenance, LanguageSpan, LanguageTag};
pub use self::polygon::{Polygon, Vertex};
pub use self::time_span::TimeSpan;

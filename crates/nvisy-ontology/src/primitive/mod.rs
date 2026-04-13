//! Primitive types used across the ontology.
//!
//! Spatial primitives (bounding boxes, polygons), temporal intervals,
//! language tags, and rendering types.

mod bounding_box;
mod bounding_box_pixel;
mod color;
mod dpi;
mod language_tag;
mod polygon;
mod time_span;

pub use self::bounding_box::BoundingBox;
pub use self::bounding_box_pixel::BoundingBoxPixel;
pub use self::color::Color;
pub use self::dpi::Dpi;
pub use self::language_tag::LanguageTag;
pub use self::polygon::{Polygon, Vertex};
pub use self::time_span::TimeSpan;

//! 2D coordinate geometry: axis-aligned bounding boxes and
//! free-form polygons.
//!
//! [`BoundingBox`] is the floating-point form used everywhere
//! coordinates may be normalised (0.0–1.0) or sub-pixel.
//! [`IBoundingBox`] is the integer pixel form used at the rendering
//! boundary, where pixel-exact coordinates are required.
//! [`Polygon`] (with [`Vertex`]) covers rotated or non-rectangular
//! regions — typically skewed text reported by OCR.

mod bounding_box;
mod polygon;

pub use self::bounding_box::{BoundingBox, IBoundingBox};
pub use self::polygon::{Polygon, Vertex};

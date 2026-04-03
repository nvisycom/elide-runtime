//! Spatial and temporal primitive types.
//!
//! Bounding boxes, polygons, time spans, and rendering primitives used
//! across entity locations, detection, and redaction operations.

mod bounding_box;
mod bounding_box_pixel;
mod color;
mod dpi;
mod polygon;
mod time_span;

pub use self::bounding_box::BoundingBox;
pub use self::bounding_box_pixel::BoundingBoxPixel;
pub use self::color::Color;
pub use self::dpi::Dpi;
pub use self::polygon::{Polygon, Vertex};
pub use self::time_span::TimeSpan;

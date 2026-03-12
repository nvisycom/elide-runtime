//! Spatial and temporal primitive types.
//!
//! Bounding boxes, polygons, and time spans used across entity
//! locations, rendering, and redaction operations.

mod bounding_box;
mod dpi;
mod polygon;
mod time_span;

pub use self::bounding_box::{BoundingBox, BoundingBoxPixel};
pub use self::dpi::Dpi;
pub use self::polygon::{Polygon, Vertex};
pub use self::time_span::TimeSpan;

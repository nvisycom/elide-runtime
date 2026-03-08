//! Spatial and temporal primitive types.
//!
//! Bounding boxes, polygons, and time spans used across entity
//! locations, rendering, and redaction operations.

mod bounding_box;
mod dpi;
mod polygon;
mod time_span;

pub use bounding_box::{BoundingBox, BoundingBoxPixel};
pub use dpi::Dpi;
pub use polygon::{Polygon, Vertex};
pub use time_span::TimeSpan;

//! 2D coordinate geometry: axis-aligned bounding boxes (pixel and
//! normalised forms), pixel dimensions, and free-form polygons.
//!
//! - [`BoundingBox`] — floating-point pixel form used everywhere
//!   pixel coordinates may be sub-pixel (OCR output, intermediate
//!   computation).
//! - [`IBoundingBox`] — integer pixel form used at the rendering
//!   boundary, where pixel-exact coordinates are required.
//! - [`NormalizedBoundingBox`] — `[0, 1]` floating-point form
//!   used at API boundaries where pixel dimensions are unknown
//!   (vision-model outputs). Convert to [`BoundingBox`] via
//!   [`NormalizedBoundingBox::to_pixel`] once [`Dimensions`] are
//!   available.
//! - [`Dimensions`] — pixel width + height; the conversion
//!   reference between normalised and pixel coordinates.
//! - [`Polygon`] (with [`Vertex`]) — rotated or non-rectangular
//!   regions, typically skewed text reported by OCR.

mod bounding_box;
mod dimensions;
mod normalized_bounding_box;
mod polygon;

pub use self::bounding_box::{BoundingBox, IBoundingBox};
pub use self::dimensions::Dimensions;
pub use self::normalized_bounding_box::NormalizedBoundingBox;
pub use self::polygon::{Polygon, Vertex};

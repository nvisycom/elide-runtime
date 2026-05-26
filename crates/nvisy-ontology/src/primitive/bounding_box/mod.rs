//! Axis-aligned bounding boxes.
//!
//! [`BoundingBox`] is the floating-point form used everywhere
//! coordinates may be normalised (0.0–1.0) or sub-pixel.
//! [`IBoundingBox`] is the integer pixel form used at the rendering
//! boundary, where pixel-exact coordinates are required.

mod float;
mod integer;

pub use self::float::BoundingBox;
pub use self::integer::IBoundingBox;

//! Rendering-side knobs: color, resolution.
//!
//! These types are consumed at the rasterisation / output boundary
//! (image redaction overlay color, PDF / image rendering resolution).
//! They live separately from the geometry types because they're
//! producer-facing presentation choices rather than coordinates.

mod color;
mod dpi;

pub use self::color::Color;
pub use self::dpi::Dpi;

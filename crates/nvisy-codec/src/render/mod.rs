//! Rendering primitives for redaction overlays.

/// Image rendering: blur and block overlay for bounding-box regions.
#[cfg(any(feature = "png", feature = "jpeg"))]
pub mod image;

/// Text rendering: byte-offset replacement engine and cell-level masking.
pub mod text;

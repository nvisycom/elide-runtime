//! Rendering primitives for redaction overlays.

/// Redaction output types recording what was done.
pub mod output;

/// Image rendering: blur and block overlay for bounding-box regions.
#[cfg(any(feature = "png", feature = "jpeg"))]
pub mod image;

/// Text rendering: byte-offset replacement engine and cell-level masking.
pub mod text;

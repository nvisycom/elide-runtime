//! DocTR OCR backend.
//!
//! Sends images as multipart form data to a DocTR server and parses
//! word-level results with normalised-to-pixel coordinate conversion.

mod backend;
mod params;

pub use backend::DoctrBackend;
pub use params::DoctrParams;

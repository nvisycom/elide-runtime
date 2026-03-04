//! Surya OCR backend.
//!
//! Sends images as multipart form data to a Surya server and parses
//! word-level results with pixel-coordinate bounding boxes and polygons.

mod backend;
mod params;

pub use backend::SuryaBackend;
pub use params::SuryaParams;

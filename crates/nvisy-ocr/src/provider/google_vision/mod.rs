//! Google Cloud Vision API backend.
//!
//! Sends base64-encoded images to the `images:annotate` endpoint and
//! parses word-level results from the `fullTextAnnotation` response.

mod backend;
mod params;

pub use backend::GoogleVisionBackend;
pub use params::GoogleVisionParams;

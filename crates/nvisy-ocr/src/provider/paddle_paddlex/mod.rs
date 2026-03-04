//! PaddleX PP-OCRv5 backend.
//!
//! Sends images as multipart form data to a PaddleX server with
//! `returnWordBox=true` and parses word-level bounding polygons.

mod backend;
mod params;

pub use backend::PaddleXBackend;
pub use params::PaddleXParams;

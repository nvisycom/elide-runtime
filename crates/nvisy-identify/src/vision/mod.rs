//! Computer vision detection layers.

pub mod face;
pub mod object;
pub mod ocr;

pub use face::{FaceBackend, FaceDetection};
pub use object::{ObjectBackend, ObjectDetection};
pub use ocr::OcrDetection;

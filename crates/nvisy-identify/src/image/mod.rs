//! Image detection layers.

pub mod ner;
pub mod face;
pub mod object;

pub use ner::ImageNerDetection;
pub use face::{FaceBackend, FaceDetection};
pub use object::{ObjectBackend, ObjectDetection};

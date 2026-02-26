//! Detection method adapters wrapping external crates.
//!
//! Each sub-module provides a thin struct that holds an agent or engine
//! from `nvisy-rig` / `nvisy-pattern` and implements the
//! [`DetectionLayer`](crate::DetectionLayer) /
//! [`DetectionService`](crate::DetectionService) traits.

mod ner;
mod cv;
mod pattern;

pub use ner::{NerMethod, NerMethodParams};
pub use cv::CvMethod;
pub use pattern::{PatternDetection, PatternDetectionParams};

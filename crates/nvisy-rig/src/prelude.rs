//! Convenience re-exports.

pub use crate::backend::{DetectionConfig, DetectionRequest, DetectionResponse, RetryPolicy, UsageStats, UsageTracker};
pub use crate::bridge::{EntityParser, RigBackend, RigBackendConfig};
pub use crate::agent::ocr::OcrProvider;
pub use crate::agent::cv::{CvDetection, CvProvider};

//! Computer vision agent for face/plate/signature detection (VLM + CV).
//!
//! Placeholder agent — implementation deferred to a future PR.

use async_trait::async_trait;

use nvisy_core::Error;

/// A single computer-vision detection result.
#[derive(Debug, Clone)]
pub struct CvDetection {
    /// Label for the detected object (e.g. "face", "license_plate").
    pub label: String,
    /// Detection confidence (0.0 -- 1.0).
    pub confidence: f64,
    /// Bounding box: `[x, y, width, height]` in pixels.
    pub bbox: [f64; 4],
}

/// Trait for computer-vision capabilities (face/plate/signature detection).
///
/// Consumers implement this trait to supply object detection from images.
/// No rig-core types leak through this trait.
#[async_trait]
pub trait CvProvider: Send + Sync {
    /// Detect objects in an image.
    async fn detect_objects(&self, image_data: &[u8]) -> Result<Vec<CvDetection>, Error>;
}

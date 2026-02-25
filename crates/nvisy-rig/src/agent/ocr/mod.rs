//! OCR agent for vision + text extraction (VLM + OCR).
//!
//! Placeholder agent — implementation deferred to a future PR.

use async_trait::async_trait;

use nvisy_core::Error;

/// Trait for OCR capabilities that can be provided to VLM agents.
///
/// Consumers implement this trait to supply text extraction from images.
/// No rig-core types leak through this trait.
#[async_trait]
pub trait OcrProvider: Send + Sync {
    /// Extract text from an image.
    async fn extract_text(&self, image_data: &[u8]) -> Result<String, Error>;
}

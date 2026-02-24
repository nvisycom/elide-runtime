//! NER backend trait and configuration.

use serde_json::Value;

use nvisy_core::Error;

/// Configuration passed to an [`NerBackend`] implementation.
///
/// Contains only the model-agnostic parameters that every backend needs.
/// Provider-specific fields (API key, model name, etc.) belong in the
/// action's [`NerDetectionParams`](super::text::NerDetectionParams)
/// or the provider's credentials.
#[derive(Debug, Clone)]
pub struct NerConfig {
    /// Entity type labels to detect (e.g., `["PERSON", "SSN"]`).
    pub entity_types: Vec<String>,
    /// Minimum confidence score to include a detection (0.0 -- 1.0).
    pub confidence_threshold: f64,
}

/// Backend trait for NER providers.
///
/// Implementations call an external NER service (e.g. via Python, HTTP)
/// and return raw JSON results.  Entity construction from the raw dicts
/// is handled by the detection layers.
#[async_trait::async_trait]
pub trait NerBackend: Send + Sync + 'static {
    /// Detect entities in text, returning raw dicts.
    async fn detect_text(
        &self,
        text: &str,
        config: &NerConfig,
    ) -> Result<Vec<Value>, Error>;

    /// Detect entities in an image, returning raw dicts.
    async fn detect_image(
        &self,
        image_data: &[u8],
        mime_type: &str,
        config: &NerConfig,
    ) -> Result<Vec<Value>, Error>;
}

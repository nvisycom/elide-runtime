//! Image generation service wrapping rig-core's `ImageGenerationModel`.

use rig::image_generation::ImageGenerationModel as _;
use uuid::Uuid;

use crate::error::Error;

use super::base::{ImageGenModels, ImageGenProvider};

/// Configuration for the image generation service.
#[derive(Debug, Clone)]
pub struct ImageGenConfig {
    /// Model name (e.g. `"dall-e-3"`, `"gpt-image-1"`).
    pub model: String,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Maximum retries for transient HTTP errors (default: 3).
    pub max_retries: u32,
}

impl Default for ImageGenConfig {
    fn default() -> Self {
        Self {
            model: "dall-e-3".to_owned(),
            width: 1024,
            height: 1024,
            max_retries: 3,
        }
    }
}

/// Image generation service wrapping rig-core image generation providers.
///
/// Currently only supports OpenAI (dall-e-3, gpt-image-1).
pub struct ImageGenService {
    id: Uuid,
    inner: ImageGenModels,
    config: ImageGenConfig,
}

impl ImageGenService {
    /// Create a new image generation service for the given provider.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Client`] if client construction fails.
    pub fn new(provider: &ImageGenProvider, config: ImageGenConfig) -> Result<Self, Error> {
        let inner =
            ImageGenModels::from_provider(provider, &config.model, config.max_retries)?;

        Ok(Self {
            id: Uuid::now_v7(),
            inner,
            config,
        })
    }

    /// Unique identifier for this service instance (UUIDv7).
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Generate an image from a text prompt, returning raw image bytes.
    #[tracing::instrument(
        skip_all,
        fields(service_id = %self.id, prompt_len = prompt.len()),
    )]
    pub async fn generate(&self, prompt: &str) -> Result<Vec<u8>, Error> {
        let image = match &self.inner {
            ImageGenModels::OpenAi(model) => {
                let response = model
                    .image_generation_request()
                    .prompt(prompt)
                    .width(self.config.width)
                    .height(self.config.height)
                    .send()
                    .await?;
                response.image
            }
        };

        tracing::info!(image_len = image.len(), "image generation complete");

        Ok(image)
    }
}

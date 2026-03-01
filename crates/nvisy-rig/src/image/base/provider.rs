//! Provider-erased dispatch enum and constructor for image generation models.

use reqwest_middleware::ClientWithMiddleware;
use rig::image_generation::ImageGenerationModel as _;
use rig::providers::openai;

use crate::backend::{AuthenticatedProvider, build_http_client};
use crate::error::Error;

/// Supported providers for image generation.
///
/// Currently only OpenAI supports image generation.
#[derive(Debug, Clone)]
pub enum ImageGenProvider {
    /// OpenAI (dall-e-3, gpt-image-1, etc.)
    OpenAi(AuthenticatedProvider),
}

impl ImageGenProvider {
    /// Create an OpenAI image generation provider.
    pub fn openai(api_key: &str, model: &str) -> Self {
        Self::OpenAi(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// The model name for this provider.
    pub fn model(&self) -> &str {
        match self {
            Self::OpenAi(p) => &p.model,
        }
    }
}

/// Provider-erased dispatch enum for image generation models.
pub(crate) enum ImageGenModels {
    OpenAi(openai::image_generation::ImageGenerationModel<ClientWithMiddleware>),
}

impl ImageGenModels {
    /// Build the appropriate image generation model for the given provider.
    pub fn from_provider(
        provider: &ImageGenProvider,
        model: &str,
        max_retries: u32,
    ) -> Result<Self, Error> {
        let http = build_http_client(max_retries);

        match provider {
            ImageGenProvider::OpenAi(p) => {
                let client = p.openai_client(http)?;
                let model =
                    <openai::image_generation::ImageGenerationModel<_>>::make(&client, model);
                Ok(Self::OpenAi(model))
            }
        }
    }
}

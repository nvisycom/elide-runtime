//! Convenience re-exports.

pub use crate::backend::{ImageFormat, ImageInput, OcrBackend, OcrConfig, OcrRegion};
pub use crate::provider::{DoctrBackend, PaddleXBackend, SuryaBackend};

#[cfg(feature = "aws")]
pub use crate::cloud::AwsTextractBackend;
#[cfg(feature = "azure")]
pub use crate::cloud::AzureDocaiBackend;
#[cfg(feature = "google")]
pub use crate::cloud::GoogleVisionBackend;

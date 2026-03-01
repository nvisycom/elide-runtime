//! Image services: image generation.

#[cfg(feature = "image")]
mod base;

#[cfg(feature = "image")]
pub use base::ImageGenProvider;

#[cfg(feature = "image")]
#[cfg_attr(docsrs, doc(cfg(feature = "image")))]
pub mod generate;

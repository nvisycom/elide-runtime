#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod backend;
mod engine;
mod local;

#[cfg(any(feature = "aws", feature = "azure", feature = "google"))]
mod cloud;

#[doc(hidden)]
pub mod prelude;

pub use backend::{Backend, ImageFormat, ImageInput, ImageOutput, ImageRegion, RunParams};
pub use engine::Engine;

/// Re-exports all OCR backend implementations and their parameter types.
pub mod provider {
    pub use crate::local::{
        DoctrBackend, DoctrParams,
        PaddleXBackend, PaddleXParams,
        SuryaBackend, SuryaParams,
    };

    #[cfg(feature = "aws")]
    #[cfg_attr(docsrs, doc(cfg(feature = "aws")))]
    pub use crate::cloud::{AwsTextractBackend, AwsTextractParams};
    #[cfg(feature = "azure")]
    #[cfg_attr(docsrs, doc(cfg(feature = "azure")))]
    pub use crate::cloud::{AzureDocaiBackend, AzureDocaiParams};
    #[cfg(feature = "google")]
    #[cfg_attr(docsrs, doc(cfg(feature = "google")))]
    pub use crate::cloud::{GoogleVisionBackend, GoogleVisionParams};
}

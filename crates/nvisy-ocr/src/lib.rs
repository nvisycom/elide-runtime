#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod backend;
pub mod provider;

#[cfg(any(feature = "aws", feature = "azure", feature = "google"))]
pub mod cloud;

pub mod prelude;

pub use backend::{ImageFormat, ImageInput, OcrBackend, OcrConfig, OcrRegion};
pub use provider::{DoctrBackend, PaddleXBackend, SuryaBackend};

#[cfg(feature = "aws")]
pub use cloud::AwsTextractBackend;
#[cfg(feature = "azure")]
pub use cloud::AzureDocaiBackend;
#[cfg(feature = "google")]
pub use cloud::GoogleVisionBackend;

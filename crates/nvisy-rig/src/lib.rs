#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod backend;
pub mod bridge;
pub(crate) mod agent;

#[doc(hidden)]
pub mod prelude;

pub use backend::{DetectionConfig, DetectionRequest, DetectionResponse};
pub use bridge::{EntityParser, RigBackend, RigBackendConfig};

// Tool-provider traits for consumers to implement.
pub use agent::ocr::OcrProvider;
pub use agent::cv::{CvDetection, CvProvider};

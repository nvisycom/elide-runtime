#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod backend;
pub mod bridge;
pub mod error;
pub(crate) mod agent;

#[doc(hidden)]
pub mod prelude;

pub use backend::{DetectionConfig, DetectionRequest, DetectionResponse};
pub use bridge::EntityParser;
pub use error::Error;

pub use agent::{
    AuthenticatedProvider, BaseAgentConfig, ContextWindow, Provider,
    UnauthenticatedProvider,
    CvAgent, CvDetection, CvProvider, NerAgent,
    OcrAgent, OcrOutput, OcrProvider, OcrTextRegion,
    RawCvEntities, RawCvEntity, RawEntities, RawEntity,
    RawOcrEntity,
};

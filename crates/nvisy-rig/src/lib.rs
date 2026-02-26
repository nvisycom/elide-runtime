#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod backend;
pub mod bridge;
pub mod error;
mod agent;

#[doc(hidden)]
pub mod prelude;

pub use agent::BaseAgentConfig;
pub use backend::{
    AuthenticatedProvider, ContextWindow,
    DetectionConfig, DetectionRequest, DetectionResponse,
    Provider, UnauthenticatedProvider, UsageStats, UsageTracker,
};
pub use error::Error;

pub use agent::{
    CvAgent, CvDetection, CvEntities, CvEntity, CvProvider,
    NerAgent, NerEntities, NerEntity,
    OcrAgent, OcrEntity, OcrOutput, OcrProvider, OcrTextRegion,
};

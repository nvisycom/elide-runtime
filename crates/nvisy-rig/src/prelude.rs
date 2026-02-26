//! Convenience re-exports.

pub use crate::backend::{
    DetectionConfig, DetectionRequest, DetectionResponse,
    UsageStats, UsageTracker,
};
pub use crate::bridge::EntityParser;
pub use crate::error::Error;
pub use crate::agent::{
    AuthenticatedProvider, BaseAgentConfig, ContextWindow, Provider,
    RetryConfig, UnauthenticatedProvider,
    CvAgent, CvDetection, CvEntities, CvEntity, CvProvider,
    NerAgent, NerEntities, NerEntity,
    OcrAgent, OcrEntity, OcrOutput, OcrProvider, OcrTextRegion,
};

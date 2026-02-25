//! Convenience re-exports.

pub use crate::backend::{
    DetectionConfig, DetectionRequest, DetectionResponse,
    UsageStats, UsageTracker,
};
pub use crate::bridge::EntityParser;
pub use crate::error::Error;
pub use crate::agent::{
    AuthenticatedProvider, BaseAgentConfig, ContextWindow, Provider,
    UnauthenticatedProvider,
    CvAgent, CvDetection, CvProvider, NerAgent,
    OcrAgent, OcrOutput, OcrProvider, OcrTextRegion,
    RawCvEntities, RawCvEntity, RawEntities, RawEntity,
    RawOcrEntity,
};

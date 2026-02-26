//! Convenience re-exports.

pub use crate::agent::BaseAgentConfig;
pub use crate::backend::{
    AuthenticatedProvider, ContextWindow,
    DetectionConfig, DetectionRequest, DetectionResponse,
    Provider, UnauthenticatedProvider, UsageStats, UsageTracker,
};
pub use crate::error::Error;
pub use crate::agent::{
    CvAgent, CvDetection, CvEntities, CvEntity, CvProvider,
    NerAgent, NerEntities, NerEntity,
    OcrAgent, OcrEntity, OcrOutput, OcrProvider, OcrTextRegion,
};

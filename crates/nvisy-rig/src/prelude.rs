//! Convenience re-exports.

pub use crate::backend::{
    DetectionConfig, DetectionRequest, DetectionResponse,
    RetryPolicy, UsageStats, UsageTracker,
};
pub use crate::bridge::{EntityParser, RigBackend, RigBackendConfig, ServiceBackend};
pub use crate::agent::{
    BaseAgentConfig, ContextWindow,
    CvAgent, CvDetection, CvProvider, NerAgent,
    OcrAgent, OcrOutput, OcrProvider, OcrTextRegion,
    RawCvEntities, RawCvEntity, RawEntities, RawEntity,
    RawOcrEntity, RawRedaction, RedactorAgent, RedactorOutput,
};

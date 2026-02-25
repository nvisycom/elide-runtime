//! Convenience re-exports.

pub use crate::backend::{
    DetectionConfig, DetectionRequest, DetectionResponse,
    RetryPolicy, UsageStats, UsageTracker,
};
pub use crate::bridge::{EntityParser, RigBackend, RigBackendConfig};
pub use crate::agent::{
    CvAgent, CvDetection, CvProvider, NerAgent,
    OcrAgent, OcrOutput, OcrProvider,
    RawCvEntities, RawCvEntity, RawEntities, RawEntity,
    RawOcrEntity, RawRedaction, RedactorAgent, RedactorOutput,
};

//! Convenience re-exports.

pub use crate::agent::{
    AgentConfig, AgentProvider, ContextWindow, CvAgent, CvDetection, CvEntities, CvEntity,
    CvProvider, DetectionConfig, DetectionRequest, DetectionResponse, KnownNerEntity, NerAgent,
    NerContext, NerEntities, NerEntity, OcrAgent, ProposedEntity, ResolvedOffsets,
    VerificationOutput, VerificationStatus, VerifiedEntity,
};
pub use crate::audio::stt::{SttConfig, SttOutput, SttService};
pub use crate::audio::tts::{TtsConfig, TtsService};
pub use crate::audio::{SttProvider, TtsProvider};
pub use crate::backend::{
    AuthenticatedProvider, UnauthenticatedProvider, UsageStats, UsageTracker,
};
pub use crate::error::Error;

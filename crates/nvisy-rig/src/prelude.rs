//! Convenience re-exports.

pub use crate::agent::AgentProvider;
pub use crate::agent::AgentConfig;
pub use crate::agent::ContextWindow;
pub use crate::agent::{
    CvAgent, CvDetection, CvEntities, CvEntity, CvProvider, KnownNerEntity, NerAgent, NerContext,
    NerEntities, NerEntity, OcrAgent, ProposedEntity, ResolvedOffsets, VerificationOutput,
    VerificationStatus, VerifiedEntity,
};
pub use crate::agent::{DetectionConfig, DetectionRequest, DetectionResponse};
pub use crate::audio::TranscribeProvider;
pub use crate::backend::{
    AuthenticatedProvider, UnauthenticatedProvider, UsageStats, UsageTracker,
};
pub use crate::error::Error;

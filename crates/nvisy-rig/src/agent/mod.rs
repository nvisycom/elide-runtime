//! Specialized LLM agents: NER (text), CV (vision), generation
//! (synthetic replacement values), and entity verification
//! (LLM-mediated check of upstream-proposed entities against an
//! image).
//!
//! Each agent composes a `BaseAgent` with domain-specific prompts
//! and optional tools. Public types are re-exported from [`crate`]
//! — consumer code should not reach into submodules.

mod base;
mod cv;
mod entity_verifier;
mod generate;
mod ner;

pub(crate) use self::base::{ALL_TYPES_HINT, BaseAgent};
pub use self::base::{
    AgentConfig, AgentProvider, AuthenticatedProvider, ContextWindow, DetectionConfig,
    DetectionRequest, UnauthenticatedProvider, UsageStats, UsageTracker,
};
pub use self::cv::{CvAgent, CvDetection, CvEntities, CvEntity, CvProvider};
pub use self::entity_verifier::{
    EntityVerifier, ProposedEntity, VerificationCandidate, VerificationOutput, VerificationStatus,
    VerifiedEntity,
};
pub use self::generate::{GenAgent, GenOutput, GenRequest, GeneratedEntity};
pub use self::ner::{
    KnownNerEntity, NerAgent, NerContext, NerEntities, NerEntity, ResolvedOffsets,
};

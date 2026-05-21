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
mod cv_verifier;
mod generate;
mod ner;
mod ner_verifier;

pub(crate) use self::base::{ALL_TYPES_HINT, BaseAgent};
pub use self::base::{
    AgentConfig, AgentProvider, AuthenticatedProvider, ContextWindow, DetectionConfig,
    UnauthenticatedProvider, UsageStats, UsageTracker, VerificationOutput, VerificationStatus,
    VerifiedEntity,
};
pub use self::cv::{CvAgent, CvDetection, CvEntities, CvEntity, CvProvider};
pub use self::cv_verifier::{CvVerifier, ProposedEntity, VerificationCandidate};
pub use self::generate::{GenAgent, GenOutput, GenRequest, GeneratedEntity};
pub use self::ner::{KnownNerEntity, NerAgent, NerCandidate, NerContext};
pub use self::ner_verifier::{NerVerifier, UnresolvedCandidatePolicy};

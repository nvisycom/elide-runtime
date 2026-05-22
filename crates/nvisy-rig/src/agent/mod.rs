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
mod cv_verify_agent;
mod generate;
mod ner;
mod ner_verify_agent;

#[cfg(any(feature = "openai-whisper", feature = "openai-tts"))]
pub(crate) use self::base::AuthenticatedProvider;
pub(crate) use self::base::{ALL_TYPES_HINT, BaseAgent, UnauthenticatedProvider};
pub use self::base::{AgentConfig, AgentProvider, DetectionConfig, UsageStats};
pub use self::cv::{CvAgent, CvDetection, CvEntity};
pub use self::cv_verify_agent::{CvVerifyAgent, VerificationCandidate};
pub use self::generate::{GenAgent, GenRequest, GeneratedEntity};
pub use self::ner::{KnownNerEntity, NerAgent, NerCandidate, NerContext};
pub use self::ner_verify_agent::{NerVerifyAgent, UnresolvedCandidatePolicy};

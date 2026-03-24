//! Specialized detection agents: NER (text), CV (vision), OCR (image-to-text),
//! and text generation (synthetic replacement values).
//!
//! Each agent composes a `BaseAgent` with domain-specific
//! prompts and optional tools. Public types are re-exported from [`crate`] —
//! consumer code should not reach into submodules.

mod base;
mod cv;
mod generate;
mod ner;
mod ocr;

pub(crate) use self::base::{ALL_TYPES_HINT, BaseAgent};
pub use self::base::{
    AgentConfig, AgentProvider, ContextWindow, DetectionConfig, DetectionRequest, DetectionResponse,
};
pub use self::cv::{CvAgent, CvDetection, CvEntities, CvEntity, CvProvider};
pub use self::generate::{GenAgent, GenOutput, GenRequest, GeneratedEntity};
pub use self::ner::{
    KnownNerEntity, NerAgent, NerContext, NerEntities, NerEntity, ResolvedOffsets,
};
pub use self::ocr::{
    OcrAgent, ProposedEntity, VerificationOutput, VerificationStatus, VerifiedEntity,
};

//! Specialized LLM agents, grouped by modality.
//!
//! Each modality (CV, NER) bundles its detect-style and
//! verify-style agents under a shared parent — they share data
//! shapes ([`cv::CvEntity`], [`ner::NerCandidate`])
//! and are meant to be used together by orchestrating pipelines.
//!
//! Cross-cutting infrastructure ([`AgentConfig`], [`AgentProvider`],
//! [`DetectionConfig`], [`UsageStats`]) is re-exported at this
//! level since every agent consumes it.

pub mod cv;
pub mod generate;
pub mod ner;

mod base;

#[cfg(any(feature = "openai-whisper", feature = "openai-tts"))]
pub(crate) use self::base::AuthenticatedProvider;
pub(crate) use self::base::{ALL_TYPES_HINT, UnauthenticatedProvider};
pub use self::base::{AgentConfig, AgentProvider, DetectionConfig, UsageStats};

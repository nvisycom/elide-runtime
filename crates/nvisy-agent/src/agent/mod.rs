//! Specialized LLM agents, grouped by modality.
//!
//! Each modality bundles its detect-style and verify-style agents
//! under a shared parent:
//!
//! - [`ner`] — text-side LLM detector + whole-audit verifier.
//! - [`vlm`] — image-side vision-language model detector + verifier.
//!
//! Cross-cutting infrastructure ([`AgentConfig`], [`AgentProvider`],
//! [`UsageStats`]) is re-exported at this level since every agent
//! consumes it.

pub mod ner;
pub mod vlm;

mod base;

#[cfg(feature = "openai-whisper")]
pub(crate) use self::base::AuthenticatedProvider;
pub(crate) use self::base::UnauthenticatedProvider;
pub use self::base::{AgentConfig, AgentProvider, UsageStats};

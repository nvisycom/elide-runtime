//! LLM-based contextual entity detection.

pub mod detection;
pub mod prompt;

pub use detection::{LlmBackend, LlmDetection, LlmDetectionParams};
pub use prompt::user_prompt;

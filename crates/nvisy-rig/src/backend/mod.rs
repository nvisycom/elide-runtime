//! LLM backend trait and configuration.

pub mod compact;
pub mod error;
pub mod metrics;
pub mod retry;

pub use compact::ContextWindow;
pub use error::ErrorMapper;
pub use metrics::{UsageStats, UsageTracker};
pub use retry::RetryPolicy;

use serde_json::Value;

use nvisy_core::Error;

/// Configuration passed to an [`LlmBackend`] implementation.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// Entity type labels to detect (e.g., `["PERSON", "SSN"]`).
    pub entity_types: Vec<String>,
    /// Minimum confidence score to include a detection (0.0 -- 1.0).
    pub confidence_threshold: f64,
    /// System prompt override (if empty, the backend uses its default).
    pub system_prompt: Option<String>,
}

/// Backend trait for LLM-based entity detection.
///
/// Implementations call an LLM service (e.g. via `rig-core`) and return
/// raw JSON results.  Entity construction from the raw dicts is handled
/// by the detection layers.
#[async_trait::async_trait]
pub trait LlmBackend: Send + Sync + 'static {
    /// Detect entities in text using an LLM, returning raw dicts.
    ///
    /// Each dict should contain: `category`, `entity_type`, `value`,
    /// `confidence`, `start_offset`, `end_offset`.
    async fn detect_text(
        &self,
        text: &str,
        config: &LlmConfig,
    ) -> Result<Vec<Value>, Error>;
}

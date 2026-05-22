//! [`VlmExtractorConfig`]: full bundle for constructing a
//! [`VlmExtractor`].
//!
//! [`VlmExtractor`]: super::VlmExtractor

use nvisy_agent::agent::{AgentConfig, AgentProvider};
use serde::{Deserialize, Serialize};

/// `[extractor.vlm]` config bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlmExtractorConfig {
    /// Enable this extractor. When `false`, the extractor is
    /// neither built nor dispatched, but the config is preserved
    /// so operators can toggle without losing it. Defaults to
    /// `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// VLM provider selection + credentials.
    pub provider: AgentProvider,
    /// Sampling/retry parameters for the VLM agent.
    #[serde(default)]
    pub agent: AgentConfig,
}

fn default_true() -> bool {
    true
}

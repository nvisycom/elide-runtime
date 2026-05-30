//! [`VlmDetection`]: full bundle for constructing the VLM-driven
//! image-modality pipeline from config alone.
//!
//! Mirrors [`LlmDetection`](super::super::llm::LlmDetection) shape
//! for parity: per-pass sub-tables ([`detect`], [`verify`]) use
//! the same presence-and-flag pattern.
//!
//! - sub-table absent → disabled (no agent allocated)
//! - sub-table present with `enabled = false` → disabled (config
//!   preserved for later toggling)
//! - sub-table present with `enabled = true` (the default when the
//!   flag is omitted) → enabled
//!
//! [`provider`]: VlmDetection::provider
//! [`detect`]: VlmDetection::detect
//! [`verify`]: VlmDetection::verify

use nvisy_agent::agent::{AgentConfig, AgentProvider};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// VLM-specific detection settings.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct VlmDetection {
    /// Enable this recognizer. When `false`, the recognizer is
    /// neither built nor dispatched, but the config is preserved
    /// so operators can toggle without losing it. Defaults to
    /// `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Provider selection + credentials. Shared by every pass.
    pub provider: AgentProvider,
    /// Detect-pass configuration. When `None`, the detect pass is
    /// disabled — the recognizer produces no entities of its own,
    /// though verify may still run over entities from other
    /// recognizers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect: Option<VlmDetectParams>,
    /// Verify-pass configuration. When `None`, no verify pass
    /// runs. When `Some`, an extra VLM call adjudicates the
    /// merged entity set after all recognizers have run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify: Option<VlmVerifyParams>,
}

/// Configuration for the VLM detect pass.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct VlmDetectParams {
    /// Enable the detect pass. Defaults to `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Sampling and retry parameters for the detect-pass agent.
    /// Ignored when [`enabled`](Self::enabled) is `false`.
    #[serde(flatten)]
    pub agent: AgentConfig,
}

/// Configuration for the VLM verify pass.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct VlmVerifyParams {
    /// Enable the verify pass. Defaults to `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Sampling and retry parameters for the verify-pass agent.
    /// Ignored when [`enabled`](Self::enabled) is `false`.
    #[serde(flatten)]
    pub agent: AgentConfig,
}

fn default_true() -> bool {
    true
}

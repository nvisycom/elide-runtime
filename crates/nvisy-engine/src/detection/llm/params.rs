//! [`LlmDetection`]: full bundle for constructing the LLM-driven
//! NER pipeline from config alone.
//!
//! Per-pass sub-tables ([`detect`], [`verify`]) use the same
//! presence-and-flag pattern:
//!
//! - sub-table absent → disabled (no agent allocated)
//! - sub-table present with `enabled = false` → disabled (config
//!   preserved for later toggling)
//! - sub-table present with `enabled = true` (the default when the
//!   flag is omitted) → enabled
//!
//! [`unresolved_policy`] is recognizer-wide (it governs the shared
//! localizer used by the detect path), so it lives at the top
//! level alongside [`provider`] rather than on any sub-table.
//!
//! Note: bundling credentials per-recognizer here is intentional
//! short-term. A follow-up will simplify the TOML schema and route
//! provider credentials through the runtime config instead — see
//! <https://github.com/nvisycom/runtime/issues/157>.
//!
//! [`provider`]: LlmDetection::provider
//! [`detect`]: LlmDetection::detect
//! [`verify`]: LlmDetection::verify
//! [`unresolved_policy`]: LlmDetection::unresolved_policy

use nvisy_agent::agent::ner::UnresolvedCandidatePolicy;
use nvisy_agent::agent::{AgentConfig, AgentProvider};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// LLM-specific detection settings.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct LlmDetection {
    /// Enable this recognizer. When `false`, the recognizer is
    /// neither built nor dispatched, but the config is preserved
    /// so operators can toggle without losing it. Defaults to
    /// `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Provider selection + credentials. Shared by every pass.
    pub provider: AgentProvider,
    /// How to handle candidates the localizer can't uniquely place
    /// in the source text. Recognizer-wide because it governs the
    /// shared localizer used by detect. Defaults to
    /// [`UnresolvedCandidatePolicy::Drop`].
    #[serde(default)]
    pub unresolved_policy: UnresolvedCandidatePolicy,
    /// Detect-pass configuration. When `None`, the detect pass is
    /// disabled — the recognizer produces no entities of its own,
    /// though verify may still run over entities from other
    /// recognizers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect: Option<DetectParams>,
    /// Verify-pass configuration. When `None`, no verify pass
    /// runs. When `Some`, an extra LLM call adjudicates the
    /// merged entity set after all recognizers have run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify: Option<VerifyParams>,
}

/// Configuration for the detect pass: the LLM agent that drives
/// unified entity detection (open-ended discovery + per-hint
/// adjudication).
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DetectParams {
    /// Enable the detect pass. Defaults to `true` so a present
    /// sub-table with no flag is treated as "I want this on".
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Sampling and retry parameters for the detect-pass agent.
    /// Ignored when [`enabled`](Self::enabled) is `false`.
    #[serde(flatten)]
    pub agent: AgentConfig,
}

/// Configuration for the whole-audit verify pass.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct VerifyParams {
    /// Enable the verify pass. Defaults to `true` so a present
    /// sub-table with no flag is treated as "I want this on".
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

//! [`LlmDetection`]: full bundle for constructing an [`LlmRecognizer`].
//!
//! Carries everything the recognizer needs to assemble itself from
//! workflow config alone:
//!
//! - [`provider`] — which LLM to talk to (selection + credentials).
//! - [`agent`] — sampling/retry knobs for the detection agent.
//! - [`verifier`] — when `Some`, enables the two-pass verifier
//!   refinement; the agent config in the variant drives the second
//!   LLM call.
//!
//! Note: bundling credentials per-recognizer here is intentional
//! short-term. A follow-up will simplify the TOML schema and route
//! provider credentials through the runtime config instead — see
//! <https://github.com/nvisycom/runtime/issues/157>.
//!
//! [`provider`]: LlmDetection::provider
//! [`agent`]: LlmDetection::agent
//! [`verifier`]: LlmDetection::verifier

use nvisy_agent::agent::ner::UnresolvedCandidatePolicy;
use nvisy_agent::agent::{AgentConfig, AgentProvider};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// LLM-specific detection settings.
///
/// No `Default` impl: every field that drives a real LLM call
/// (provider, agent config) must be supplied by the caller.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct LlmDetection {
    /// Provider selection + credentials for the detection agent.
    pub provider: AgentProvider,
    /// Sampling and retry parameters for the detection agent.
    pub agent: AgentConfig,
    /// Enables a second LLM pass that adjusts confidence based on
    /// surrounding document context. `None` disables refinement
    /// (one LLM call per span); `Some` enables it with the carried
    /// agent config (two LLM calls per span, allowing a stricter
    /// or cheaper verifier model than detection).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verifier: Option<AgentConfig>,
    /// How to handle NER candidates that can't be uniquely
    /// localized in the source text. Defaults to
    /// [`UnresolvedCandidatePolicy::Drop`].
    #[serde(default)]
    pub unresolved_policy: UnresolvedCandidatePolicy,
}

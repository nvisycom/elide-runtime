//! [`LlmDetection`]: full bundle for constructing an [`LlmRecognizer`].
//!
//! Carries everything the recognizer needs to assemble itself from
//! config alone:
//!
//! - [`provider`] — which LLM to talk to (selection + credentials).
//! - [`agent`] — sampling/retry knobs reused for both detection and
//!   (when [`verify_pass`] is true) the verifier pass.
//! - [`verify_pass`] — `true` enables the two-pass refinement
//!   verifier using the same [`agent`] config; `false` runs
//!   localization-only verification.
//! - [`unresolved_policy`] — how the verifier handles candidates
//!   that can't be uniquely localized in the source text.
//!
//! Note: bundling credentials per-recognizer here is intentional
//! short-term. A follow-up will simplify the TOML schema and route
//! provider credentials through the runtime config instead — see
//! <https://github.com/nvisycom/runtime/issues/157>.
//!
//! [`LlmRecognizer`]: super::LlmRecognizer
//! [`provider`]: LlmDetection::provider
//! [`agent`]: LlmDetection::agent
//! [`verify_pass`]: LlmDetection::verify_pass
//! [`unresolved_policy`]: LlmDetection::unresolved_policy

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
    /// Sampling and retry parameters. Reused for both the detection
    /// pass and (when [`verify_pass`] is true) the verifier pass.
    ///
    /// [`verify_pass`]: Self::verify_pass
    pub agent: AgentConfig,
    /// `true` enables a second LLM pass that adjusts confidence
    /// based on surrounding document context (two LLM calls per
    /// span); `false` runs localization-only verification (one LLM
    /// call per span). The verifier reuses [`agent`] when enabled.
    ///
    /// [`agent`]: Self::agent
    #[serde(default)]
    pub verify_pass: bool,
    /// How to handle NER candidates that can't be uniquely
    /// localized in the source text. Defaults to
    /// [`UnresolvedCandidatePolicy::Drop`].
    #[serde(default)]
    pub unresolved_policy: UnresolvedCandidatePolicy,
}

//! Engine-wide configuration, typically deserialized from TOML.
//!
//! [`RuntimeConfig`] is the top-level configuration object containing
//! optional subsystem sections — [`EngineConfig`],
//! [`ExtractionConfig`][ec], [`DetectionConfig`][dc],
//! [`RedactionConfig`][rc].
//!
//! Per-request plan nodes (`Extraction`, `Redaction`,
//! `DeduplicationParams`, `Validation`) live alongside their
//! corresponding side's config in [`crate::detection`] or
//! [`crate::redaction`]; the per-phase modules consume them at
//! dispatch time.
//!
//! # Post-load steps
//!
//! After deserializing from TOML, callers should:
//! 1. Call [`RuntimeConfig::resolve_env`] to fill empty `api_key`
//!    fields from environment variables.
//! 2. Call [`RuntimeConfig::validate`] to check structural
//!    constraints.
//!
//! [ec]: crate::detection::ExtractionConfig
//! [dc]: crate::detection::DetectionConfig
//! [rc]: crate::redaction::RedactionConfig

use std::num::NonZeroUsize;
use std::time::Duration;

use nvisy_core::Error;
use nvisy_core::Result;
use nvisy_llm::backend::http::HttpConfig;
use semver::Version;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::detection::{DetectionConfig, ExtractionConfig};
use crate::redaction::RedactionConfig;

fn default_config_version() -> Version {
    Version::new(0, 1, 0)
}

/// Top-level pipeline configuration, typically deserialized from TOML.
///
/// Contains optional subsystem sections. The CLI layer owns the full
/// TOML shape (including `[server]`) and passes this struct downstream
/// to the engines.
///
/// Every section is load-once: the engines read this struct at
/// startup, build the per-section state behind `Arc`s on their inner
/// shared state, and never re-read it. Per-request override is not
/// supported — plans tune behaviour through their own per-phase
/// config nodes, not by resupplying a `RuntimeConfig`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Configuration schema version.
    #[serde(default = "default_config_version")]
    pub version: Version,

    /// Engine-level execution policies, networking, and resource
    /// limits.
    pub engine: Option<EngineConfig>,
    /// Extraction registry — `[extractor.ocr]`, `[extractor.stt]`
    /// sub-sections. Built once at engine startup; the `Extraction`
    /// phase config carries per-call flags only.
    pub extraction: Option<ExtractionConfig>,
    /// Detection registry — `[detection.pattern]`,
    /// `[detection.ner]` sub-sections. Built once at engine startup;
    /// the `Detection` phase config only references these by kind.
    pub detection: Option<DetectionConfig>,
    /// Deployment-wide redaction defaults — `[redaction]` section.
    /// Built once at engine startup; the per-plan `Redaction` node
    /// falls back to these for any `None` fields. Per-request
    /// override is not supported.
    pub redaction: Option<RedactionConfig>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            version: default_config_version(),
            engine: None,
            extraction: None,
            detection: None,
            redaction: None,
        }
    }
}

impl RuntimeConfig {
    /// Resource limits from the engine section, or defaults.
    #[must_use]
    pub fn effective_limits(&self) -> ResourceLimits {
        self.engine.as_ref().map(|e| e.limits).unwrap_or_default()
    }

    /// Concurrency limit from the engine section's resource limits,
    /// if configured.
    #[must_use]
    pub fn effective_concurrency(&self) -> Option<NonZeroUsize> {
        self.engine.as_ref().and_then(|e| e.limits.concurrency)
    }

    /// Validate all configuration sections.
    ///
    /// Checks structural constraints (e.g. retry/timeout ranges)
    /// using the `validator` crate. Should be called once after
    /// deserialization and after any merge.
    ///
    /// # Errors
    ///
    /// Returns a validation error listing all constraint violations.
    pub fn validate(&self) -> Result<(), Error> {
        if let Some(ref engine) = self.engine {
            engine
                .validate()
                .map_err(|e| Error::validation(format!("engine: {e}"), "config"))?;
        }
        Ok(())
    }

    /// Resolve `api_key` fields from environment variables.
    ///
    /// Placeholder: per-extractor/per-recognizer provider configs
    /// will get their own env-var resolution path in a follow-up.
    pub fn resolve_env(&mut self) {}
}

/// Engine-level configuration: networking + resource limits.
///
/// All settings are deployment-side — set once in `Nvisy.toml` by
/// the operator. Per-request overrides apply only to fields
/// explicitly noted as overridable.
#[derive(Debug, Clone, Default, Validate, Serialize, Deserialize)]
pub struct EngineConfig {
    /// Shared HTTP client configuration for all downstream API calls.
    ///
    /// Applies to OCR providers, LLM agents, STT services, and any
    /// other external HTTP dependencies. Controls timeouts, retries,
    /// and connection pooling.
    pub http: Option<HttpConfig>,

    /// Run-level resource limits (concurrency cap + timeout).
    ///
    /// Nested under `[engine.limits]` in TOML.
    #[validate(nested)]
    #[serde(default)]
    pub limits: ResourceLimits,
}

/// Hard limits on pipeline resource consumption.
///
/// Deployment-side caps applied to every
/// [`DetectionEngine::detect`][de] / [`RedactionEngine::redact`][re]
/// pass — not adjustable per-request.
///
/// [de]: crate::detection::DetectionEngine::detect
/// [re]: crate::redaction::RedactionEngine::redact
#[derive(Debug, Clone, Copy, Default, Validate, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum number of documents processed in parallel via a
    /// shared [`Semaphore`]. Server-wide; not overridable
    /// per-request. `None` means unbounded.
    ///
    /// [`Semaphore`]: tokio::sync::Semaphore
    #[serde(default)]
    pub concurrency: Option<NonZeroUsize>,

    /// Hard ceiling on total pipeline run duration.
    ///
    /// On expiry, the run-level cancellation token fires and the
    /// run is marked as timed out. `None` means no run-level
    /// timeout — rely on external supervision (k8s liveness, etc.)
    /// instead.
    ///
    /// Parses from human-friendly strings via `humantime_serde`:
    /// `"60s"`, `"5m"`, `"1h30m"`.
    #[serde(
        default,
        with = "humantime_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub run_timeout: Option<Duration>,
}

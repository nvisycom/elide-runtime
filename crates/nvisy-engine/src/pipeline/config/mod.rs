//! Pipeline runtime configuration, typically deserialized from TOML.
//!
//! [`RuntimeConfig`] is the top-level configuration object containing
//! optional subsystem sections — [`EngineConfig`],
//! [`ExtractionConfig`], [`DetectionConfig`], [`RedactionConfig`].
//!
//! Per-request plan nodes ([`Detection`], [`Extraction`],
//! [`Redaction`], [`DeduplicationParams`]) live alongside their
//! corresponding config sections; the per-phase `crate::detection`,
//! `crate::phases::extraction`, … modules consume them at dispatch time.
//!
//! # Post-load steps
//!
//! After deserializing from TOML, callers should:
//! 1. Call [`RuntimeConfig::resolve_env`] to fill empty `api_key` fields
//!    from environment variables.
//! 2. Call [`RuntimeConfig::validate`] to check structural constraints.

mod detection;
mod engine;
mod extraction;
mod redaction;
mod validation;

use std::num::NonZeroUsize;

use nvisy_core::Error;
pub use nvisy_toolkit::deduplication::DeduplicationParams;
use semver::Version;
use serde::{Deserialize, Serialize};

pub use self::detection::{Detection, DetectionConfig, NerBackend, NerDetection, PatternDetection};
pub use self::engine::{EngineConfig, ResourceLimits};
#[cfg(feature = "image")]
pub use self::extraction::OcrExtractorConfig;
#[cfg(feature = "audio")]
pub use self::extraction::SttExtractorConfig;
pub use self::extraction::{
    AudioPlan, Extraction, ExtractionConfig, ImagePlan, TabularPlan, TextPlan,
};
pub use self::redaction::{Redaction, RedactionConfig};
pub use self::validation::Validation;

fn default_config_version() -> Version {
    Version::new(0, 1, 0)
}

/// Top-level pipeline configuration, typically deserialized from TOML.
///
/// Contains optional subsystem sections. The CLI layer owns the full
/// TOML shape (including `[server]`) and passes this struct downstream
/// to the engine.
///
/// Every section is load-once: the engine reads this struct at
/// startup, builds the per-section state behind `Arc`s on its inner
/// shared state, and never re-reads it. Per-request override is not
/// supported — plans tune behaviour through their own per-phase
/// config nodes ([`Extraction`], [`Detection`], [`Redaction`], …),
/// not by resupplying a `RuntimeConfig`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Configuration schema version.
    #[serde(default = "default_config_version")]
    pub version: Version,

    /// Engine-level execution policies, networking, and resource limits.
    pub engine: Option<EngineConfig>,
    /// Extraction registry — `[extractor.ocr]`, `[extractor.stt]`
    /// sub-sections. Built once at engine startup; the `Extraction`
    /// phase config carries per-call flags only.
    pub extraction: Option<ExtractionConfig>,
    /// Detection registry — `[detection.pattern]`, `[detection.ner]`
    /// sub-sections. Built once at engine startup; the `Detection`
    /// phase config only references these by kind.
    pub detection: Option<DetectionConfig>,
    /// Deployment-wide redaction defaults — `[redaction]` section.
    /// Built once at engine startup; the per-plan `Redaction`
    /// node falls back to these for any `None` fields. Per-request
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
        use validator::Validate;
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

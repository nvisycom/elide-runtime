//! Pipeline runtime configuration, typically deserialized from TOML.
//!
//! [`RuntimeConfig`] is the top-level configuration object containing
//! optional subsystem sections — [`EngineSection`],
//! [`ExtractionSection`], and [`DetectionSection`].
//!
//! Per-request overrides are supported via [`RuntimeConfig::merge`],
//! which replaces entire sections (not individual fields) when the
//! override provides a non-`None` value — with the exception of
//! `extractor` and `recognizer`, which are built once at engine
//! startup and never per-request-overridden.
//!
//! # Post-load steps
//!
//! After deserializing from TOML, callers should:
//! 1. Call [`RuntimeConfig::resolve_env`] to fill empty `api_key` fields
//!    from environment variables.
//! 2. Call [`RuntimeConfig::validate`] to check structural constraints.

mod engine;
mod validate;

use std::num::NonZeroUsize;

use semver::Version;
use serde::{Deserialize, Serialize};

pub use self::engine::{CacheConfig, EngineSection, ResourceLimits};
use crate::detection::DetectionSection;
use crate::extraction::ExtractionSection;
use crate::redaction::RedactionSection;

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
/// supported — workflows tune behaviour through their own per-phase
/// config nodes (`Extraction`, `Detection`, `Redaction`, …), not by
/// resupplying a `RuntimeConfig`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Configuration schema version.
    #[serde(default = "default_config_version")]
    pub version: Version,

    /// Engine-level execution policies, networking, and resource limits.
    pub engine: Option<EngineSection>,
    /// Extraction registry — `[extraction.ocr]`, `[extraction.stt]`
    /// sub-sections. Built once at engine startup; the `Extraction`
    /// phase config carries per-call flags only.
    pub extraction: Option<ExtractionSection>,
    /// Detection registry — `[detection.llm]`, `[detection.ner]`,
    /// `[detection.pattern]`, `[detection.vlm]` sub-sections. Built
    /// once at engine startup; the `Detection` phase config only
    /// references these by kind.
    pub detection: Option<DetectionSection>,
    /// Deployment-wide redaction defaults — `[redaction]` section.
    /// Built once at engine startup; the per-workflow `Redaction`
    /// node falls back to these for any `None` fields. Per-request
    /// override is not supported.
    pub redaction: Option<RedactionSection>,
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
    /// Returns `true` if all optional sections are `None`.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.engine.is_none()
            && self.extraction.is_none()
            && self.detection.is_none()
            && self.redaction.is_none()
    }

    /// Resource limits from the engine section, or defaults.
    #[must_use]
    pub fn effective_limits(&self) -> ResourceLimits {
        self.engine.as_ref().map(|e| e.limits).unwrap_or_default()
    }

    /// Concurrency limit from the engine section, if configured.
    #[must_use]
    pub fn effective_concurrency(&self) -> Option<NonZeroUsize> {
        self.engine.as_ref().and_then(|e| e.concurrency)
    }
}

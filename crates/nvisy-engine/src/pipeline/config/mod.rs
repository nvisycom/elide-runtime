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

use semver::Version;
use serde::{Deserialize, Serialize};

pub use self::engine::{CacheConfig, EngineSection, ResourceLimits};
use crate::detection::DetectionSection;
use crate::extraction::ExtractionSection;
use crate::pipeline::ConcurrencyPolicy;
use crate::redaction::RedactionDefaults;

fn default_config_version() -> Version {
    Version::new(0, 1, 0)
}

/// Top-level pipeline configuration, typically deserialized from TOML.
///
/// Contains optional subsystem sections. The CLI layer owns the full
/// TOML shape (including `[server]`) and passes this struct downstream
/// to the engine.
///
/// # Merge semantics
///
/// [`RuntimeConfig::merge`] replaces entire sections — if the override
/// provides a non-`None` section, it wins completely. Fields within a
/// section are not merged individually. `extractor` and `recognizer`
/// are NOT overridable per-request: both are built once at engine
/// startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Configuration schema version.
    #[serde(default = "default_config_version")]
    pub version: Version,

    /// Engine-level execution policies, networking, and resource limits.
    pub engine: Option<EngineSection>,
    /// Extraction registry — `[extraction.ocr]`, `[extraction.stt]`,
    /// `[extraction.vlm]` sub-sections. Built once at engine startup;
    /// the `Extraction` phase config carries per-call flags only.
    pub extraction: Option<ExtractionSection>,
    /// Detection registry — `[detection.llm]`, `[detection.nlp]`,
    /// `[detection.pattern]` sub-sections. Built once at engine
    /// startup; the `Detection` phase config only references these
    /// by kind.
    pub detection: Option<DetectionSection>,
    /// Server-wide redaction defaults — `[redaction]` section.
    /// `Redaction` phase config falls back to these for any `None`
    /// fields.
    pub redaction: Option<RedactionDefaults>,
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

    /// Concurrency policy from the engine section, if configured.
    #[must_use]
    pub fn effective_concurrency(&self) -> Option<ConcurrencyPolicy> {
        self.engine.as_ref().and_then(|e| e.concurrency)
    }

    /// Merge with per-request overrides.
    ///
    /// Non-`None` sections in `overrides` replace the corresponding
    /// section in `self`; `None` sections fall back to `self`. The
    /// version is taken from the base config. `extraction` and
    /// `detection` are intentionally NOT overridable: each is built
    /// once at engine startup so per-request dispatch stays cheap.
    /// Per-request override would force a rebuild (model loads,
    /// HTTP client setup) and defeat the amortization.
    #[must_use]
    pub fn merge(&self, overrides: &RuntimeConfig) -> RuntimeConfig {
        RuntimeConfig {
            version: self.version.clone(),
            engine: overrides.engine.clone().or_else(|| self.engine.clone()),
            extraction: self.extraction.clone(),
            detection: self.detection.clone(),
            redaction: overrides
                .redaction
                .clone()
                .or_else(|| self.redaction.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_overrides_present_sections() {
        let base = RuntimeConfig {
            engine: Some(EngineSection {
                http: Some(nvisy_http::HttpConfig {
                    max_retries: 3,
                    timeout: std::time::Duration::from_secs(120),
                    connect_timeout: std::time::Duration::from_secs(10),
                    idle_timeout: std::time::Duration::from_secs(90),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let overrides = RuntimeConfig {
            engine: Some(EngineSection {
                http: Some(nvisy_http::HttpConfig {
                    max_retries: 1,
                    timeout: std::time::Duration::from_secs(30),
                    connect_timeout: std::time::Duration::from_secs(5),
                    idle_timeout: std::time::Duration::from_secs(60),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let merged = base.merge(&overrides);
        assert_eq!(merged.engine.unwrap().http.unwrap().max_retries, 1);
    }

    #[test]
    fn merge_falls_back_to_base() {
        let base = RuntimeConfig {
            engine: Some(EngineSection {
                http: Some(nvisy_http::HttpConfig {
                    max_retries: 3,
                    timeout: std::time::Duration::from_secs(120),
                    connect_timeout: std::time::Duration::from_secs(10),
                    idle_timeout: std::time::Duration::from_secs(90),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let overrides = RuntimeConfig::default();
        let merged = base.merge(&overrides);
        assert_eq!(merged.engine.unwrap().http.unwrap().max_retries, 3);
    }
}

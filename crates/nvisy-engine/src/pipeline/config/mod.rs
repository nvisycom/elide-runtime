//! Pipeline runtime configuration, typically deserialized from TOML.
//!
//! [`RuntimeConfig`] is the top-level configuration object containing
//! optional subsystem sections — [`EngineSection`],
//! [`ExtractorSection`], and [`RecognizerSection`].
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
use crate::detection::RecognizerSection;
use crate::extraction::ExtractorSection;
use crate::pipeline::{ConcurrencyPolicy, TimeoutPolicy};
use crate::redaction::RedactorDefaults;

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
    /// Extractor registry — `[extractor.visual]`, `[extractor.audial]`
    /// sub-sections. Built once at engine startup; workflow
    /// `Extraction` nodes carry per-call flags only.
    pub extractor: Option<ExtractorSection>,
    /// Recognizer registry — `[recognizer.llm]`, `[recognizer.nlp]`,
    /// `[recognizer.pattern]` sub-sections. Built once at engine
    /// startup; workflow `Detection` nodes only reference these by
    /// kind.
    pub recognizer: Option<RecognizerSection>,
    /// Server-wide redaction defaults — `[redactor]` section.
    /// Workflow `Redaction` nodes fall back to these for any
    /// `None` fields.
    pub redactor: Option<RedactorDefaults>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            version: default_config_version(),
            engine: None,
            extractor: None,
            recognizer: None,
            redactor: None,
        }
    }
}

impl RuntimeConfig {
    /// Default timeout policy from the engine section, if configured.
    #[must_use]
    pub fn default_timeout(&self) -> Option<&TimeoutPolicy> {
        self.engine.as_ref().and_then(|e| e.timeout.as_ref())
    }

    /// Returns `true` if all optional sections are `None`.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.engine.is_none()
            && self.extractor.is_none()
            && self.recognizer.is_none()
            && self.redactor.is_none()
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
    /// version is taken from the base config. `extractor` and
    /// `recognizer` are intentionally NOT overridable: each is built
    /// once at engine startup so per-request dispatch stays cheap.
    /// Per-request override would force a rebuild (model loads,
    /// HTTP client setup) and defeat the amortization.
    #[must_use]
    pub fn merge(&self, overrides: &RuntimeConfig) -> RuntimeConfig {
        RuntimeConfig {
            version: self.version.clone(),
            engine: overrides.engine.clone().or_else(|| self.engine.clone()),
            extractor: self.extractor.clone(),
            recognizer: self.recognizer.clone(),
            redactor: overrides.redactor.clone().or_else(|| self.redactor.clone()),
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

//! Pipeline runtime configuration, typically deserialized from TOML.
//!
//! [`RuntimeConfig`] is the top-level configuration object containing
//! optional sections for each subsystem — [`EngineSection`],
//! [`OcrSection`], [`LlmSection`], [`SttSection`], and [`TtsSection`].
//!
//! Per-request overrides are supported via [`RuntimeConfig::merge`],
//! which replaces entire sections (not individual fields) when the
//! override provides a non-`None` value.
//!
//! # Post-load steps
//!
//! After deserializing from TOML, callers should:
//! 1. Call [`RuntimeConfig::resolve_env`] to fill empty `api_key` fields
//!    from environment variables.
//! 2. Call [`RuntimeConfig::validate`] to check structural constraints.

mod engine;
mod subsystem;
mod validate;

use nvisy_ontology::workflow::{ConcurrencyPolicy, RetryPolicy, TimeoutPolicy};
use semver::Version;
use serde::{Deserialize, Serialize};

pub use self::engine::{CacheConfig, EngineSection, ResourceLimits};
pub use self::subsystem::{LlmSection, OcrSection, SttSection, TtsSection};

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
/// section are not merged individually.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Configuration schema version.
    #[serde(default = "default_config_version")]
    pub version: Version,

    /// Engine-level execution policies, networking, and resource limits.
    pub engine: Option<EngineSection>,
    /// OCR subsystem (optical character recognition).
    pub ocr: Option<OcrSection>,
    /// LLM subsystem (language model inference).
    pub llm: Option<LlmSection>,
    /// STT subsystem (speech-to-text transcription).
    pub stt: Option<SttSection>,
    /// TTS subsystem (text-to-speech generation).
    pub tts: Option<TtsSection>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            version: default_config_version(),
            engine: None,
            ocr: None,
            llm: None,
            stt: None,
            tts: None,
        }
    }
}

impl RuntimeConfig {
    /// Default retry policy from the engine section, if configured.
    #[must_use]
    pub fn default_retry(&self) -> Option<&RetryPolicy> {
        self.engine.as_ref().and_then(|e| e.retry.as_ref())
    }

    /// Default timeout policy from the engine section, if configured.
    #[must_use]
    pub fn default_timeout(&self) -> Option<&TimeoutPolicy> {
        self.engine.as_ref().and_then(|e| e.timeout.as_ref())
    }

    /// Returns `true` if all optional sections are `None`.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.engine.is_none()
            && self.ocr.is_none()
            && self.llm.is_none()
            && self.stt.is_none()
            && self.tts.is_none()
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
    /// version is taken from the base config.
    #[must_use]
    pub fn merge(&self, overrides: &RuntimeConfig) -> RuntimeConfig {
        RuntimeConfig {
            version: self.version.clone(),
            engine: overrides.engine.clone().or_else(|| self.engine.clone()),
            ocr: overrides.ocr.clone().or_else(|| self.ocr.clone()),
            llm: overrides.llm.clone().or_else(|| self.llm.clone()),
            stt: overrides.stt.clone().or_else(|| self.stt.clone()),
            tts: overrides.tts.clone().or_else(|| self.tts.clone()),
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
                http: Some(nvisy_provider::http::HttpConfig {
                    max_retries: 3,
                    timeout_secs: 120,
                    connect_timeout_secs: 10,
                    idle_timeout_secs: 90,
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let overrides = RuntimeConfig {
            engine: Some(EngineSection {
                http: Some(nvisy_provider::http::HttpConfig {
                    max_retries: 1,
                    timeout_secs: 30,
                    connect_timeout_secs: 5,
                    idle_timeout_secs: 60,
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
                http: Some(nvisy_provider::http::HttpConfig {
                    max_retries: 3,
                    timeout_secs: 120,
                    connect_timeout_secs: 10,
                    idle_timeout_secs: 90,
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let overrides = RuntimeConfig::default();
        let merged = base.merge(&overrides);
        assert_eq!(merged.engine.unwrap().http.unwrap().max_retries, 3);
    }

    #[test]
    fn fill_key_skips_nonempty() {
        let mut key = "existing".to_string();
        // Point at an env var that almost certainly doesn't exist.
        super::validate::fill_key_from_env(&mut key, "NVISY_TEST_NONEXISTENT_VAR_12345");
        assert_eq!(key, "existing");
    }

    #[test]
    fn fill_key_leaves_empty_when_env_missing() {
        let mut key = String::new();
        super::validate::fill_key_from_env(&mut key, "NVISY_TEST_NONEXISTENT_VAR_12345");
        assert!(key.is_empty());
    }
}

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

use semver::Version;
use serde::{Deserialize, Serialize};

pub use self::engine::{EngineSection, ResourceLimits};
pub use self::subsystem::{LlmSection, OcrSection, SttSection, TtsSection};
use crate::graph::{RetryPolicy, TimeoutPolicy};

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

    /// Merge with per-request overrides.
    ///
    /// Non-`None` sections in `overrides` replace the corresponding section
    /// in `self`; `None` sections fall back to `self`. The version is taken
    /// from the base config.
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
    fn empty_toml_parses_to_defaults() {
        let config: RuntimeConfig = toml::from_str("").unwrap();
        assert_eq!(config.version, Version::new(0, 1, 0));
        assert!(config.engine.is_none());
        assert!(config.ocr.is_none());
        assert!(config.llm.is_none());
        assert!(config.stt.is_none());
        assert!(config.tts.is_none());
    }

    #[test]
    fn http_section_parses_under_engine() {
        let toml = r#"
            [engine.http]
            max_retries = 5
            timeout_secs = 60
            connect_timeout_secs = 5
            idle_timeout_secs = 30
        "#;
        let config: RuntimeConfig = toml::from_str(toml).unwrap();
        let http = config.engine.unwrap().http.unwrap();
        assert_eq!(http.max_retries, 5);
        assert_eq!(http.timeout_secs, 60);
    }

    #[test]
    fn ocr_provider_section_parses() {
        let toml = r#"
            [ocr.provider]
            kind = "surya"
            base_url = "http://localhost:8001"
        "#;
        let config: RuntimeConfig = toml::from_str(toml).unwrap();
        assert!(config.ocr.is_some());
        assert!(config.ocr.unwrap().provider.is_some());
    }

    #[test]
    fn ocr_policy_section_parses() {
        let toml = r#"
            [ocr.policy]
            confidence_threshold = 0.5
        "#;
        let config: RuntimeConfig = toml::from_str(toml).unwrap();
        let ocr = config.ocr.unwrap();
        let policy = ocr.policy.unwrap();
        assert!((policy.confidence_threshold - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn engine_retry_section_parses() {
        let toml = r#"
            [engine.retry]
            max_retries = 3
            delay_ms = 500
            backoff = "fixed"
        "#;
        let config: RuntimeConfig = toml::from_str(toml).unwrap();
        let engine = config.engine.unwrap();
        let retry = engine.retry.unwrap();
        assert_eq!(retry.max_retries, 3);
        assert_eq!(retry.delay_ms, 500);
    }

    #[test]
    fn engine_timeout_section_parses() {
        let toml = r#"
            [engine.timeout]
            duration_ms = 30000
            on_timeout = "fail"
        "#;
        let config: RuntimeConfig = toml::from_str(toml).unwrap();
        let engine = config.engine.unwrap();
        let timeout = engine.timeout.unwrap();
        assert_eq!(timeout.duration_ms, 30000);
    }

    #[test]
    fn llm_policy_section_parses() {
        let toml = r#"
            [llm.policy]
            temperature = 0.1
            max_tokens = 4096
            max_retries = 3
        "#;
        let config: RuntimeConfig = toml::from_str(toml).unwrap();
        let llm = config.llm.unwrap();
        let policy = llm.policy.unwrap();
        assert!((policy.temperature - 0.1).abs() < f64::EPSILON);
        assert_eq!(policy.max_tokens, 4096);
        assert_eq!(policy.max_retries, 3);
    }

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
    fn validate_rejects_zero_channel_buffer() {
        let config = RuntimeConfig {
            engine: Some(EngineSection {
                limits: ResourceLimits {
                    channel_buffer: 0,
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_accepts_valid_config() {
        let config = RuntimeConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn version_parses_as_semver() {
        let toml = r#"version = "1.2.3""#;
        let config: RuntimeConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.version, Version::new(1, 2, 3));
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

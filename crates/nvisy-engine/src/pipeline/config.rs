use nvisy_http::HttpConfig;
use nvisy_rig::agent::{AgentConfig, AgentProvider};
use nvisy_rig::audio::{SttProvider, TtsProvider};
use serde::{Deserialize, Serialize};

use crate::graph::{ConcurrencyPolicy, RetryPolicy, TimeoutPolicy};

/// OCR subsystem configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrSection {
    /// Whether the OCR subsystem is active (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// OCR provider selection + connection settings.
    pub provider: Option<nvisy_ocr::OcrProvider>,
    /// OCR runtime parameters (confidence thresholds, etc.).
    pub policy: Option<nvisy_ocr::RunParams>,
}

impl Default for OcrSection {
    fn default() -> Self {
        Self {
            enabled: true,
            provider: None,
            policy: None,
        }
    }
}

/// LLM subsystem configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSection {
    /// Whether the LLM subsystem is active (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// LLM provider selection + connection settings.
    pub provider: Option<AgentProvider>,
    /// LLM sampling and retry parameters.
    pub policy: Option<AgentConfig>,
}

impl Default for LlmSection {
    fn default() -> Self {
        Self {
            enabled: true,
            provider: None,
            policy: None,
        }
    }
}

/// Speech-to-text subsystem configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttSection {
    /// Whether the STT subsystem is active (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// STT provider selection + connection settings.
    pub provider: Option<SttProvider>,
}

impl Default for SttSection {
    fn default() -> Self {
        Self {
            enabled: true,
            provider: None,
        }
    }
}

/// Text-to-speech subsystem configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsSection {
    /// Whether the TTS subsystem is active (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// TTS provider selection + connection settings.
    pub provider: Option<TtsProvider>,
}

impl Default for TtsSection {
    fn default() -> Self {
        Self {
            enabled: true,
            provider: None,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Engine-level policies (retry, timeout) and HTTP client settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngineSection {
    /// Default retry policy for graph nodes.
    pub retry: Option<RetryPolicy>,
    /// Default timeout policy for graph nodes.
    pub timeout: Option<TimeoutPolicy>,
    /// HTTP client configuration for downstream calls.
    pub http: Option<HttpConfig>,
    /// Default concurrpiency limit for graph execution.
    /// Overridden by [`Graph::concurrency`] if set.
    ///
    /// [`Graph::concurrency`]: crate::graph::Graph::concurrency
    #[serde(default)]
    pub concurrency: Option<ConcurrencyPolicy>,
    /// Maximum wall-clock time (in milliseconds) for a single pipeline run.
    #[serde(default)]
    pub run_timeout_ms: Option<u64>,
    /// Buffer size for bounded MPSC channels between nodes.
    #[serde(default)]
    pub channel_buffer: Option<usize>,
}

/// Pipeline subsystem configuration.
///
/// Contains engine policies and provider settings for OCR, LLM, STT, TTS,
/// and HTTP. Deserialized from the non-`[server]` sections of the TOML file.
/// The CLI layer owns the full TOML shape (including `[server]`) and passes
/// this struct downstream.
fn default_config_version() -> u32 {
    1
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Configuration schema version.
    #[serde(default = "default_config_version")]
        pub version: u32,
    /// Engine-level policies and HTTP client.
    pub engine: Option<EngineSection>,
    /// OCR subsystem configuration.
    pub ocr: Option<OcrSection>,
    /// LLM subsystem configuration.
    pub llm: Option<LlmSection>,
    /// Speech-to-text subsystem configuration.
    pub stt: Option<SttSection>,
    /// Text-to-speech subsystem configuration.
    pub tts: Option<TtsSection>,
}

impl RuntimeConfig {
    /// Merge with per-request overrides.
    ///
    /// Non-`None` sections in `overrides` replace the corresponding section
    /// in `self`; `None` sections fall back to `self`.
    #[must_use]
    pub fn merge(&self, overrides: &RuntimeConfig) -> RuntimeConfig {
        RuntimeConfig {
            version: overrides.version,
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
        assert_eq!(config.version, 1);
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
    fn enabled_defaults_to_true() {
        let toml = r#"
            [ocr.provider]
            kind = "surya"
            base_url = "http://localhost:8001"
        "#;
        let config: RuntimeConfig = toml::from_str(toml).unwrap();
        assert!(config.ocr.unwrap().enabled);
    }

    #[test]
    fn enabled_can_be_disabled() {
        let toml = r#"
            [ocr]
            enabled = false
        "#;
        let config: RuntimeConfig = toml::from_str(toml).unwrap();
        assert!(!config.ocr.unwrap().enabled);
    }

    #[test]
    fn merge_overrides_present_sections() {
        let base = RuntimeConfig {
            engine: Some(EngineSection {
                http: Some(nvisy_http::HttpConfig {
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
                http: Some(nvisy_http::HttpConfig {
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
                http: Some(nvisy_http::HttpConfig {
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
}

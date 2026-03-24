//! Pipeline runtime configuration, typically deserialized from TOML.
//!
//! [`RuntimeConfig`] is the top-level configuration object containing
//! optional sections for each subsystem — [`EngineSection`],
//! [`OcrSection`], [`LlmSection`], [`SttSection`], and [`TtsSection`].
//!
//! Per-request overrides are supported via [`RuntimeConfig::merge`],
//! which replaces entire sections (not individual fields) when the
//! override provides a non-`None` value.

use nvisy_http::HttpConfig;
use nvisy_rig::agent::{AgentConfig, AgentProvider};
use nvisy_rig::audio::{SttProvider, TtsProvider};
use serde::{Deserialize, Serialize};

use crate::graph::{ConcurrencyPolicy, RetryPolicy, TimeoutPolicy};

/// Hard limits on pipeline resource consumption.
///
/// These values cap the overall duration and internal buffer sizes
/// for a single pipeline run. They are read once during
/// [`Engine::run`](super::Engine::run) and cannot be changed mid-run.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Hard ceiling on total pipeline run duration, in milliseconds.
    ///
    /// If a run exceeds this limit, the cancellation token is triggered
    /// and the run is marked as timed out. Individual node timeouts
    /// (via [`TimeoutPolicy`]) are independent of this limit.
    /// `None` means no run-level timeout.
    #[serde(default)]
    pub run_timeout_ms: Option<u64>,

    /// Buffer size for bounded MPSC channels between pipeline nodes.
    ///
    /// Controls backpressure: a larger buffer allows faster producers
    /// to run ahead of slower consumers. Defaults to 256.
    #[serde(default = "ResourceLimits::default_channel_buffer")]
    pub channel_buffer: usize,
}

impl ResourceLimits {
    const fn default_channel_buffer() -> usize {
        256
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            run_timeout_ms: None,
            channel_buffer: Self::default_channel_buffer(),
        }
    }
}

/// Engine-level execution policies, networking, and resource limits.
///
/// Controls default behavior for all pipeline runs unless overridden
/// by per-request or per-graph configuration.
///
/// # Field groups
///
/// **Execution policies** — applied to graph nodes that lack their own:
/// - [`retry`](Self::retry) — automatic retry on transient failures.
/// - [`timeout`](Self::timeout) — per-node wall-clock deadline.
/// - [`concurrency`](Self::concurrency) — limits parallel node execution.
///
/// **Networking:**
/// - [`http`](Self::http) — shared HTTP client settings (timeouts, retries,
///   connection pooling) for all downstream API calls (OCR, LLM, STT, etc.).
///
/// **Resource limits:**
/// - [`limits`](Self::limits) — run timeout and channel buffer size.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngineSection {
    /// Default retry policy applied to nodes without an explicit one.
    ///
    /// Controls max retries, delay, and backoff strategy for transient
    /// failures. Overridden by [`GraphNode::retry`](crate::graph::GraphNode::retry)
    /// when set on an individual node. See [`RetryPolicy`] for details.
    pub retry: Option<RetryPolicy>,

    /// Default timeout policy applied to nodes without an explicit one.
    ///
    /// Sets a per-node wall-clock deadline and behavior on expiry
    /// (fail or skip). Overridden by [`GraphNode::timeout`](crate::graph::GraphNode::timeout)
    /// when set on an individual node. See [`TimeoutPolicy`] for details.
    pub timeout: Option<TimeoutPolicy>,

    /// Default concurrency limit for graph execution.
    ///
    /// Caps the number of graph nodes that may execute in parallel via
    /// a [`tokio::sync::Semaphore`]. Overridden by
    /// [`Graph::concurrency`](crate::graph::Graph::concurrency)
    /// when set on the graph itself.
    #[serde(default)]
    pub concurrency: Option<ConcurrencyPolicy>,

    /// Shared HTTP client configuration for all downstream API calls.
    ///
    /// Applies to OCR providers, LLM agents, STT/TTS services, and any
    /// other external HTTP dependencies. Controls timeouts, retries,
    /// and connection pooling.
    pub http: Option<HttpConfig>,

    /// Run-level resource limits (timeout, channel buffer size).
    ///
    /// Nested under `[engine.limits]` in TOML.
    #[serde(default)]
    pub limits: ResourceLimits,
}

/// OCR subsystem configuration.
///
/// Controls the optical character recognition provider and its runtime
/// parameters (confidence thresholds, language hints, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrSection {
    /// Whether the OCR subsystem is active (default: `true`).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// OCR provider selection and connection settings.
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
///
/// Controls the language model provider used for NER, OCR verification,
/// and other inference tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSection {
    /// Whether the LLM subsystem is active (default: `true`).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// LLM provider selection and connection settings.
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
///
/// Controls the STT provider used by [`AudialExtraction`](crate::graph::AudialExtraction)
/// nodes for audio transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttSection {
    /// Whether the STT subsystem is active (default: `true`).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// STT provider selection and connection settings.
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
///
/// Controls the TTS provider for audio generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsSection {
    /// Whether the TTS subsystem is active (default: `true`).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// TTS provider selection and connection settings.
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

fn default_config_version() -> u32 {
    1
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
    /// Configuration schema version (default: 1).
    #[serde(default = "default_config_version")]
    pub version: u32,

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
    /// Merge with per-request overrides.
    ///
    /// Non-`None` sections in `overrides` replace the corresponding section
    /// in `self`; `None` sections fall back to `self`. The version is taken
    /// from the base config.
    #[must_use]
    pub fn merge(&self, overrides: &RuntimeConfig) -> RuntimeConfig {
        RuntimeConfig {
            version: self.version,
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

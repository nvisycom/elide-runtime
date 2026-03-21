//! TOML configuration file types.
//!
//! Every struct in this module maps 1:1 to a section in `Nvisy.toml` and
//! exists solely for [`serde::Deserialize`]. CLI flags and fully-resolved
//! types live in [`super::server`] and [`super::middleware`].
//!
//! # File layout
//!
//! ```toml
//! [server]                        # → ServerSection
//! [server.observability]          # → ObservabilitySection
//! [server.middleware]             # → MiddlewareSection
//! [server.middleware.cors]        # → CorsConfig
//!
//! [engine]                        # ┐
//! [engine.http]                   # │
//! [ocr] / [llm] / [stt] / [tts]  # ┘ → RuntimeConfig (flattened)
//! ```

use std::net::IpAddr;
use std::path::PathBuf;

use nvisy_engine::pipeline::RuntimeConfig;
use serde::Deserialize;

/// Log output format for `tracing_subscriber`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// Structured JSON lines (default). Best for log aggregation pipelines.
    #[default]
    Json,
    /// Human-readable text. Best for local development.
    Text,
}

/// `[server.observability]` — logging level and format.
///
/// The `level` field accepts any valid [`tracing_subscriber::EnvFilter`]
/// directive (e.g. `"info"`, `"nvisy_server=debug,tower_http=trace"`).
/// `RUST_LOG` always takes priority when set.
#[derive(Debug, Clone, Deserialize)]
pub struct ObservabilitySection {
    /// Filter directive. Defaults to `"info"`.
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Output format. Defaults to JSON.
    #[serde(default)]
    pub format: LogFormat,
}

impl Default for ObservabilitySection {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: LogFormat::default(),
        }
    }
}

fn default_log_level() -> String {
    "info".to_owned()
}

/// `[server.middleware.cors]` — CORS policy.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CorsConfig {
    /// Origins allowed to make cross-origin requests.
    /// An empty list (or omitted) means permissive (all origins).
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    /// `Access-Control-Max-Age` for preflight responses, in seconds.
    pub max_age_secs: Option<u64>,
}

/// `[server.middleware]` — body limits, request timeout, and CORS.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MiddlewareSection {
    /// Maximum request body size in MiB for axum extractors. Default: 4.
    pub body_limit_mb: Option<usize>,
    /// Per-request timeout in seconds. Default: 300.
    pub request_timeout_secs: Option<u64>,
    /// CORS policy. Omit for permissive defaults.
    pub cors: Option<CorsConfig>,
}

/// `[server]` — network binding, storage, observability, and middleware.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ServerSection {
    /// Listen address (e.g. `127.0.0.1`, `0.0.0.0`).
    pub host: Option<IpAddr>,
    /// TCP port number.
    pub port: Option<u16>,
    /// Graceful shutdown timeout in seconds.
    pub shutdown_timeout: Option<u64>,
    /// Directory for content and context storage.
    pub data_dir: Option<PathBuf>,
    /// Logging configuration.
    pub observability: Option<ObservabilitySection>,
    /// HTTP middleware configuration.
    pub middleware: Option<MiddlewareSection>,
}

/// Top-level TOML file shape.
///
/// Combines the `[server]` section with all runtime subsystem sections
/// (`[engine]`, `[ocr]`, `[llm]`, `[stt]`, `[tts]`) which are flattened
/// into a [`RuntimeConfig`].
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileConfig {
    /// Server and infrastructure settings.
    pub server: Option<ServerSection>,
    /// Engine and provider subsystem settings.
    #[serde(flatten)]
    pub inner: RuntimeConfig,
}

impl FileConfig {
    /// Loads configuration from a TOML file at `path`.
    ///
    /// Returns defaults if the file does not exist. Returns an error if
    /// the file exists but cannot be read or parsed.
    pub fn from_file(path: &PathBuf) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(path)?;
        let config = toml::from_str(&contents)?;
        Ok(config)
    }
}

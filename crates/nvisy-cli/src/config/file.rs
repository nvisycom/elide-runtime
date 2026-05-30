//! TOML configuration file types.
//!
//! Every struct in this module maps 1:1 to a section in `Nvisy.toml` and
//! exists solely for [`Deserialize`]. CLI flags and fully-resolved
//! types live in [`server`] and [`middleware`].
//!
//! [`Deserialize`]: serde::Deserialize
//! [`server`]: super::server
//! [`middleware`]: super::middleware

use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

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
/// The `level` field accepts any valid [`EnvFilter`] directive (e.g.
/// `"info"`, `"nvisy_server=debug,tower_http=trace"`). `RUST_LOG`
/// always takes priority when set.
///
/// [`EnvFilter`]: tracing_subscriber::EnvFilter
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
    /// `Access-Control-Max-Age` for preflight responses. Parses
    /// human-readable durations (`"1h"`, `"3600s"`).
    #[serde(default, with = "humantime_serde")]
    pub max_age: Option<Duration>,
}

/// `[server.middleware]` — body limits, request timeout, and CORS.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MiddlewareSection {
    /// Maximum request body size in MiB for axum extractors. Default: 4.
    pub body_limit_mb: Option<usize>,
    /// Per-request timeout. Parses human-readable durations
    /// (`"5m"`, `"300s"`). Default: 5m.
    #[serde(default, with = "humantime_serde")]
    pub request_timeout: Option<Duration>,
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
    /// Graceful shutdown timeout. Parses human-readable durations
    /// (`"30s"`, `"1m"`).
    #[serde(default, with = "humantime_serde")]
    pub shutdown_timeout: Option<Duration>,
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
/// (`[engine]`, `[ocr]`, `[llm]`, `[stt]`) which are flattened
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
        let contents = fs::read_to_string(path)?;
        let config = toml::from_str(&contents)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Nvisy.example.toml` is the source of truth for the documented
    /// schema. If it stops parsing, the docs lie. Loaded from the
    /// workspace root via `CARGO_MANIFEST_DIR`.
    #[test]
    fn example_toml_parses() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("Nvisy.example.toml");
        let config = FileConfig::from_file(&path).expect("Nvisy.example.toml must parse");

        // Sanity: the top-level shape resolved.
        assert!(
            config.server.is_some(),
            "[server] section should be present"
        );
        assert!(
            config.inner.engine.is_some(),
            "[engine] section should be present"
        );
        assert!(
            config.inner.extraction.is_some(),
            "[extraction.*] sections should be present"
        );
        assert!(
            config.inner.detection.is_some(),
            "[detection.*] sections should be present"
        );
        assert!(
            config.inner.redaction.is_some(),
            "[redaction] section should be present"
        );
    }
}

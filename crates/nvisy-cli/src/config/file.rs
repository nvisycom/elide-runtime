//! TOML file configuration types.
//!
//! These structs map directly to the TOML file shape and are only used
//! for deserialization. CLI flags and resolved types live in their
//! respective modules (`server.rs`, `middleware.rs`).

use std::path::PathBuf;

use nvisy_engine::RuntimeConfig;
use serde::Deserialize;
use std::net::IpAddr;

/// Log output format.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Json,
    Text,
}

/// `[server.observability]` section of the TOML configuration file.
#[derive(Debug, Clone, Deserialize)]
pub struct ObservabilitySection {
    #[serde(default = "default_log_level")]
    pub level: String,
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

/// CORS configuration in TOML.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CorsConfig {
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    pub max_age_secs: Option<u64>,
}

/// `[server.middleware]` section of the TOML configuration file.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MiddlewareSection {
    pub body_limit_mb: Option<usize>,
    pub request_timeout_secs: Option<u64>,
    pub cors: Option<CorsConfig>,
}

/// `[server]` section of the TOML configuration file.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ServerSection {
    pub host: Option<IpAddr>,
    pub port: Option<u16>,
    pub shutdown_timeout: Option<u64>,
    pub data_dir: Option<PathBuf>,
    pub observability: Option<ObservabilitySection>,
    pub middleware: Option<MiddlewareSection>,
}

/// Full TOML file shape: `[server]` + all runtime subsystem sections.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileConfig {
    pub server: Option<ServerSection>,
    #[serde(flatten)]
    pub inner: RuntimeConfig,
}

impl FileConfig {
    /// Load from a TOML file, or return defaults if the file doesn't exist.
    pub fn from_file(path: &PathBuf) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(path)?;
        let config = toml::from_str(&contents)?;
        Ok(config)
    }
}

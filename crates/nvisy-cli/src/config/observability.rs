//! Process-level observability configuration: log filter directive and
//! output format. Initialised via [`init`] at startup.

use serde::Deserialize;
use tracing_subscriber::EnvFilter;

/// `[server.observability]` — process logging.
///
/// The `level` field accepts any valid [`EnvFilter`] directive (e.g.
/// `"info"`, `"nvisy_server=debug,tower_http=trace"`). `RUST_LOG`
/// always takes priority when set.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservabilityConfig {
    /// Filter directive. Defaults to `"info"`.
    #[serde(default = "default_level")]
    pub level: String,
    /// Output format. Defaults to JSON.
    #[serde(default)]
    pub format: LogFormat,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            level: default_level(),
            format: LogFormat::default(),
        }
    }
}

fn default_level() -> String {
    "info".to_owned()
}

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

/// Initialise the global `tracing` subscriber from `config`.
///
/// `RUST_LOG` takes precedence over [`ObservabilityConfig::level`]
/// when set.
pub fn init(config: &ObservabilityConfig) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.level));
    let subscriber = tracing_subscriber::fmt().with_env_filter(filter);
    match config.format {
        LogFormat::Json => subscriber.json().init(),
        LogFormat::Text => subscriber.init(),
    }
}

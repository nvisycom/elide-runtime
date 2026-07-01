//! Process-level observability configuration types.
//!
//! Data types only; the binary that initialises `tracing`
//! consumes these (see the CLI's observability init).

use serde::Deserialize;

/// `[server.observability]`: process logging.
///
/// The `level` field accepts any valid `tracing_subscriber`
/// `EnvFilter` directive (e.g. `"info"`,
/// `"nvisy_server=debug,tower_http=trace"`). Binaries typically
/// let `RUST_LOG` take priority over this when set.
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// Structured JSON lines (default). Best for log aggregation pipelines.
    #[default]
    Json,
    /// Human-readable text. Best for local development.
    Text,
}

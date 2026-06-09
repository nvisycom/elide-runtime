//! CLI configuration management.
//!
//! TOML file is the source of truth; CLI flags override a small set
//! of network / lifecycle fields. The resolved [`AppConfig`] is what
//! the rest of the binary consumes.
//!
//! # Architecture
//!
//! ```text
//! Cli (clap)        --config <path>, --host, --port, --shutdown-timeout, --data-dir
//! └── Overrides     thin CLI overlay applied to ServerConfig
//!
//! AppConfig (TOML)
//! ├── server: ServerConfig            [server] + nested .observability / .middleware
//! └── runtime: RuntimeConfig          [engine], [extraction.*], [detection.*], [redaction]
//! ```
//!
//! [`Cli::load`] reads the file, merges CLI overrides, and returns
//! the resolved [`AppConfig`].

pub mod middleware;
pub mod observability;
mod server;

use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use clap::{Args, Parser};
use nvisy_engine::pipeline::RuntimeConfig;
use serde::Deserialize;

pub use self::server::ServerConfig;

/// Top-level CLI entry point.
#[derive(Debug, Parser)]
#[command(name = "nvisy-server", version, about = "nvisy API server")]
pub struct Cli {
    /// Path to a TOML configuration file.
    #[arg(long, env = "NVISY_CONFIG", default_value = "Nvisy.toml")]
    pub config: PathBuf,

    /// Server network and lifecycle overrides.
    #[command(flatten)]
    pub overrides: Overrides,
}

/// CLI overrides for [`ServerConfig`]. Each field, when `Some`,
/// replaces the corresponding TOML value.
///
/// All env vars are `NVISY_`-prefixed so they can't collide with
/// generic shell defaults (`HOST`, `PORT`, etc.).
#[derive(Debug, Clone, Args)]
pub struct Overrides {
    /// Host address to bind the server to.
    #[arg(long, env = "NVISY_HOST")]
    pub host: Option<IpAddr>,

    /// TCP port number for the server to listen on.
    #[arg(short = 'p', long, env = "NVISY_PORT")]
    pub port: Option<u16>,

    /// Graceful shutdown timeout. Human-readable duration
    /// (`"30s"`, `"1m"`).
    #[arg(long, env = "NVISY_SHUTDOWN_TIMEOUT", value_parser = humantime::parse_duration)]
    pub shutdown_timeout: Option<Duration>,

    /// Directory for data storage (content, contexts).
    #[arg(long, env = "NVISY_DATA_DIR")]
    pub data_dir: Option<PathBuf>,
}

impl Overrides {
    /// Apply each `Some(_)` field to `server`, leaving `None` fields
    /// at their loaded values.
    pub fn merge_into(self, server: &mut ServerConfig) {
        if let Some(host) = self.host {
            server.host = host;
        }
        if let Some(port) = self.port {
            server.port = port;
        }
        if let Some(timeout) = self.shutdown_timeout {
            server.shutdown_timeout = timeout;
        }
        if let Some(dir) = self.data_dir {
            server.data_dir = dir;
        }
    }
}

/// Resolved top-level configuration: server settings + engine
/// subsystem settings, all merged from TOML + CLI overrides.
///
/// `deny_unknown_fields` catches typos at the top level (e.g.
/// `[serer]` instead of `[server]`). Each nested struct denies
/// unknown fields too so typos inside a section also fail loudly.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppConfig {
    /// Server, observability, and middleware configuration.
    #[serde(default)]
    pub server: ServerConfig,
    /// Engine and provider subsystem settings. Flattened so its
    /// sections (`[engine]`, `[extraction.*]`, ...) sit at the
    /// TOML root alongside `[server]`. The top-level `version`
    /// key the TOML carries is consumed here by `RuntimeConfig`.
    #[serde(flatten)]
    pub runtime: RuntimeConfig,
}

impl Cli {
    /// Read the TOML file, apply CLI overrides, run runtime
    /// validation, and return the resolved [`AppConfig`].
    ///
    /// Missing TOML file resolves to defaults (everything from CLI
    /// + built-ins).
    pub fn load(self) -> anyhow::Result<AppConfig> {
        let mut config = read_toml(&self.config)?;
        self.overrides.merge_into(&mut config.server);
        config.runtime.resolve_env();
        config
            .runtime
            .validate()
            .map_err(|e| anyhow::anyhow!("invalid configuration: {}", e.message()))?;
        Ok(config)
    }
}

fn read_toml(path: &Path) -> anyhow::Result<AppConfig> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let contents = fs::read_to_string(path)
        .with_context(|| format!("reading config file `{}`", path.display()))?;
    let config = toml::from_str(&contents)
        .with_context(|| format!("parsing config file `{}`", path.display()))?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    /// `Nvisy.example.toml` is the source of truth for the documented
    /// schema. If it stops parsing, the docs lie.
    #[test]
    fn example_toml_parses() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("Nvisy.example.toml");
        let contents = fs::read_to_string(&path).expect("Nvisy.example.toml exists");
        let config: AppConfig = toml::from_str(&contents).expect("Nvisy.example.toml parses");

        assert!(config.runtime.engine.is_some(), "[engine] should be set");
        assert!(
            config.runtime.extraction.is_some(),
            "[extraction.*] should be set"
        );
        assert!(
            config.runtime.detection.is_some(),
            "[detection.*] should be set"
        );
        assert!(
            config.runtime.redaction.is_some(),
            "[redaction] should be set"
        );
    }

    #[test]
    fn defaults_are_sensible() {
        let config = ServerConfig::default();
        assert_eq!(config.host, IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        assert_eq!(config.port, 8080);
        assert_eq!(config.shutdown_timeout, Duration::from_secs(30));
    }

    #[test]
    fn overrides_merge_into_server() {
        let mut config = ServerConfig::default();
        let overrides = Overrides {
            host: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            port: Some(9090),
            shutdown_timeout: Some(Duration::from_secs(60)),
            data_dir: Some(PathBuf::from("/tmp/x")),
        };
        overrides.merge_into(&mut config);
        assert_eq!(config.host, IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(config.port, 9090);
        assert_eq!(config.shutdown_timeout, Duration::from_secs(60));
        assert_eq!(config.data_dir, PathBuf::from("/tmp/x"));
    }

    #[test]
    fn empty_overrides_preserve_config() {
        let mut config = ServerConfig::default();
        let original_port = config.port;
        let overrides = Overrides {
            host: None,
            port: None,
            shutdown_timeout: None,
            data_dir: None,
        };
        overrides.merge_into(&mut config);
        assert_eq!(config.port, original_port);
    }

    #[test]
    fn toml_parses_humantime_durations() {
        let toml_src = r#"
            [server]
            shutdown_timeout = "45s"
            [server.middleware]
            request_timeout = "10m"
        "#;
        let config: AppConfig = toml::from_str(toml_src).expect("parses");
        assert_eq!(config.server.shutdown_timeout, Duration::from_secs(45));
        assert_eq!(
            config.server.middleware.request_timeout,
            Some(Duration::from_secs(600))
        );
    }

    #[test]
    fn cli_parses_humantime_duration() {
        let parsed = humantime::parse_duration("45s").expect("parses");
        assert_eq!(parsed, Duration::from_secs(45));
        let parsed = humantime::parse_duration("1m").expect("parses");
        assert_eq!(parsed, Duration::from_secs(60));
    }

    #[test]
    fn missing_file_yields_defaults() {
        let path = PathBuf::from("/this/file/definitely/does/not/exist.toml");
        let config = read_toml(&path).expect("missing file resolves to defaults");
        assert_eq!(config.server.port, 8080);
    }

    #[test]
    fn typo_in_server_section_is_rejected() {
        let toml_src = r#"
            [server]
            prot = 8080
        "#;
        let result: Result<AppConfig, _> = toml::from_str(toml_src);
        assert!(result.is_err(), "unknown field `prot` must fail");
    }
}

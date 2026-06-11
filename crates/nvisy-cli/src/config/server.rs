//! `[server]` — network binding, lifecycle, observability, middleware.

use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

use super::middleware::MiddlewareConfig;
use super::observability::ObservabilityConfig;

/// `[server]` — fully-resolved server configuration.
///
/// Loaded from TOML with built-in defaults; CLI flags override
/// individual fields via [`super::Overrides::merge_into`].
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    /// Bind address. Use `127.0.0.1` for localhost only, `0.0.0.0`
    /// for all interfaces. Defaults to `0.0.0.0`.
    #[serde(default = "default_host")]
    pub host: IpAddr,
    /// TCP port number. Defaults to `8080`.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Graceful shutdown timeout. Parses human-readable durations
    /// (`"30s"`, `"1m"`). Defaults to 30 seconds.
    #[serde(default = "default_shutdown_timeout", with = "humantime_serde")]
    pub shutdown_timeout: Duration,
    /// Directory for content and context storage. Defaults to
    /// `$TMPDIR/nvisy-server-data`.
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    /// Process logging configuration.
    #[serde(default)]
    pub observability: ObservabilityConfig,
    /// HTTP middleware configuration.
    #[serde(default)]
    pub middleware: MiddlewareConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            shutdown_timeout: default_shutdown_timeout(),
            data_dir: default_data_dir(),
            observability: ObservabilityConfig::default(),
            middleware: MiddlewareConfig::default(),
        }
    }
}

impl ServerConfig {
    /// Socket address derived from `host` + `port`.
    #[must_use]
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }
}

fn default_host() -> IpAddr {
    IpAddr::V4(Ipv4Addr::UNSPECIFIED)
}

fn default_port() -> u16 {
    8080
}

fn default_shutdown_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_data_dir() -> PathBuf {
    env::temp_dir().join("nvisy-server-data")
}

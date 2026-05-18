//! Server network and lifecycle configuration.
//!
//! [`ServerConfig`] captures CLI flags and environment variables for the
//! server's network binding and data directory. [`ResolvedServer`] is the
//! fully-resolved form produced by merging CLI → TOML → defaults.
//!
//! # Precedence
//!
//! CLI flags and environment variables take priority over TOML values.
//! If neither is provided, a built-in default is used.
//!
//! | Setting          | CLI flag             | Env var            | Default                    |
//! |------------------|----------------------|--------------------|----------------------------|
//! | Host             | `--host`             | `HOST`             | `0.0.0.0`                  |
//! | Port             | `-p` / `--port`      | `PORT`             | `8080`                     |
//! | Shutdown timeout | `--shutdown-timeout` | `SHUTDOWN_TIMEOUT` | `30` s                     |
//! | Data directory   | `--data-dir`         | `DATA_DIR`         | `$TMPDIR/nvisy-server-data`|

use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use clap::Args;

use super::file::{LogFormat, ServerSection};

/// CLI flags for server network and lifecycle settings.
///
/// All fields are optional — when absent, resolution falls through to the
/// TOML `[server]` section and finally to built-in defaults.
#[derive(Debug, Clone, Args)]
pub struct ServerConfig {
    /// Host address to bind the server to.
    ///
    /// Use `127.0.0.1` for localhost only, `0.0.0.0` for all interfaces.
    #[arg(long, env = "HOST")]
    pub host: Option<IpAddr>,

    /// TCP port number for the server to listen on.
    #[arg(short = 'p', long, env = "PORT")]
    pub port: Option<u16>,

    /// Maximum time in seconds to wait for graceful shutdown.
    ///
    /// During shutdown, the server stops accepting new connections and waits
    /// for existing requests to complete before forcefully terminating.
    #[arg(long, env = "SHUTDOWN_TIMEOUT")]
    pub shutdown_timeout: Option<u64>,

    /// Directory for data storage (content, contexts).
    #[arg(long, env = "DATA_DIR")]
    pub data_dir: Option<PathBuf>,
}

impl ServerConfig {
    /// Merge CLI flags with TOML `[server]` values and built-in defaults.
    pub fn resolve(&self, toml: &Option<ServerSection>) -> ResolvedServer {
        let toml = toml.as_ref();
        let obs = toml
            .and_then(|s| s.observability.clone())
            .unwrap_or_default();

        ResolvedServer {
            host: self
                .host
                .or_else(|| toml.and_then(|s| s.host))
                .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
            port: self
                .port
                .or_else(|| toml.and_then(|s| s.port))
                .unwrap_or(8080),
            shutdown_timeout: self
                .shutdown_timeout
                .or_else(|| toml.and_then(|s| s.shutdown_timeout))
                .unwrap_or(30),
            data_dir: self
                .data_dir
                .clone()
                .or_else(|| toml.and_then(|s| s.data_dir.clone()))
                .unwrap_or_else(|| env::temp_dir().join("nvisy-server-data")),
            log_level: obs.level,
            log_format: obs.format,
        }
    }
}

/// Fully resolved server settings with no `Option`s.
///
/// Produced by [`ServerConfig::resolve`] after merging all configuration
/// sources. Safe to use directly without further fallback logic.
#[derive(Debug, Clone)]
pub struct ResolvedServer {
    /// Bind address.
    pub host: IpAddr,
    /// Bind port.
    pub port: u16,
    /// Graceful shutdown timeout in seconds.
    pub shutdown_timeout: u64,
    /// Directory for content and context storage.
    pub data_dir: PathBuf,
    /// Tracing filter directive (e.g. `"info"`, `"nvisy_server=debug"`).
    pub log_level: String,
    /// Log output format.
    pub log_format: LogFormat,
}

impl ResolvedServer {
    /// Returns the socket address for binding.
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }

    /// Returns the graceful shutdown timeout as a [`Duration`].
    pub fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.shutdown_timeout)
    }
}

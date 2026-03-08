//! HTTP server network and lifecycle configuration.
//!
//! # Environment Variables
//!
//! - `HOST` — Server host address (default: `0.0.0.0`)
//! - `PORT` — Server port (default: `8080`)
//! - `SHUTDOWN_TIMEOUT` — Graceful shutdown timeout in seconds (default: `30`)

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use clap::Args;

use super::file::{ObservabilitySection, ServerSection};

/// HTTP server network and lifecycle configuration.
///
/// CLI flags override values from the `[server]` section of the TOML file.
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
    /// Resolve server settings: CLI flags → TOML `[server]` → defaults.
    pub fn resolve(&self, toml: &Option<ServerSection>) -> ResolvedServer {
        let toml = toml.as_ref();
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
                .unwrap_or_else(|| std::env::temp_dir().join("nvisy-server-data")),
            observability: toml
                .and_then(|s| s.observability.clone())
                .unwrap_or_default(),
        }
    }
}

/// Fully resolved server settings with no `Option`s.
#[derive(Debug, Clone)]
pub struct ResolvedServer {
    pub host: IpAddr,
    pub port: u16,
    pub shutdown_timeout: u64,
    pub data_dir: PathBuf,
    pub observability: ObservabilitySection,
}

impl ResolvedServer {
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }

    pub fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.shutdown_timeout)
    }
}

//! HTTP server network and lifecycle configuration.
//!
//! # Environment Variables
//!
//! - `HOST` — Server host address (default: `0.0.0.0`)
//! - `PORT` — Server port (default: `8080`)
//! - `DATA_DIR` — Data storage directory (content, contexts)
//! - `SHUTDOWN_TIMEOUT` — Graceful shutdown timeout in seconds (default: `30`)

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use clap::Args;

/// HTTP server network and lifecycle configuration.
///
/// Controls how the server binds to network interfaces, where
/// data is stored, and graceful shutdown behavior.
#[derive(Debug, Clone, Args)]
pub struct ServerConfig {
    /// Host address to bind the server to.
    ///
    /// Use `127.0.0.1` for localhost only, `0.0.0.0` for all interfaces.
    #[arg(long, env = "HOST", default_value_t = IpAddr::V4(Ipv4Addr::UNSPECIFIED))]
    pub host: IpAddr,

    /// TCP port number for the server to listen on.
    #[arg(short = 'p', long, env = "PORT", default_value_t = 8080)]
    pub port: u16,

    /// Directory for data storage (content, contexts).
    ///
    /// Defaults to `$TMPDIR/nvisy-server-data` if not set.
    #[arg(long, env = "DATA_DIR")]
    pub data_dir: Option<PathBuf>,

    /// Maximum time in seconds to wait for graceful shutdown.
    ///
    /// During shutdown, the server stops accepting new connections and waits
    /// for existing requests to complete before forcefully terminating.
    #[arg(long, env = "SHUTDOWN_TIMEOUT", default_value_t = 30)]
    pub shutdown_timeout: u64,
}

impl ServerConfig {
    /// Returns the socket address for server binding.
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }

    /// Returns the data directory, falling back to a temp directory.
    pub fn data_dir(&self) -> PathBuf {
        self.data_dir
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join("nvisy-server-data"))
    }

    /// Returns the graceful shutdown timeout as a [`Duration`].
    pub fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.shutdown_timeout)
    }
}

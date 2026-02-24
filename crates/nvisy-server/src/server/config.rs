//! CLI configuration parsed from command-line arguments and environment
//! variables via [`clap`].
//!
//! All fields have sensible defaults and can be overridden by environment
//! variables (`HOST`, `PORT`, `RUST_LOG`, etc.) or CLI flags.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use clap::Parser;

use crate::middleware::recovery::RecoveryConfig;
use crate::middleware::security::SecurityConfig;
use crate::middleware::specification::OpenApiConfig;

/// nvisy API server.
#[derive(Debug, Parser)]
#[command(name = "nvisy-server", version, about)]
pub struct ServerConfig {
    /// Address to bind the HTTP listener to.
    #[arg(long, env = "HOST", default_value_t = IpAddr::V4(Ipv4Addr::UNSPECIFIED))]
    pub host: IpAddr,

    /// Port to bind the HTTP listener to.
    #[arg(long, env = "PORT", default_value_t = 8080)]
    pub port: u16,

    /// Directory for temporary content storage.
    ///
    /// Defaults to `$TMPDIR/nvisy-server-content` if not set.
    #[arg(long, env = "CONTENT_DIR")]
    pub content_dir: Option<PathBuf>,

    /// Tracing filter directive (e.g. `info`, `nvisy_server=debug`).
    #[arg(long, env = "RUST_LOG", default_value = "info")]
    pub log_level: String,

    /// Maximum request body size in bytes.
    #[arg(long, env = "BODY_LIMIT_BYTES", default_value_t = 50 * 1024 * 1024)]
    pub body_limit_bytes: usize,

    /// Per-request timeout in seconds.
    #[arg(long, env = "REQUEST_TIMEOUT_SECS", default_value_t = 300)]
    pub request_timeout_secs: u64,
}

impl ServerConfig {
    /// Returns the socket address to bind the listener to.
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }

    /// Returns the content directory, falling back to a temp directory.
    pub fn content_dir(&self) -> PathBuf {
        self.content_dir
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join("nvisy-server-content"))
    }

    /// Builds a [`SecurityConfig`] from the parsed CLI values.
    pub fn security_config(&self) -> SecurityConfig {
        SecurityConfig {
            body_limit_bytes: self.body_limit_bytes,
        }
    }

    /// Builds a [`RecoveryConfig`] from the parsed CLI values.
    pub fn recovery_config(&self) -> RecoveryConfig {
        RecoveryConfig {
            request_timeout: self.request_timeout_secs,
        }
    }

    /// Returns the default [`OpenApiConfig`].
    pub fn open_api_config(&self) -> OpenApiConfig {
        OpenApiConfig::default()
    }
}

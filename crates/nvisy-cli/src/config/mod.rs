//! CLI configuration management.
//!
//! This module defines the complete CLI configuration hierarchy:
//!
//! ```text
//! Cli
//! ├── server: ServerConfig         # Host, port, content directory
//! ├── body_limit_bytes: usize      # Extractor body limit (default: 2 MiB)
//! ├── file_body_limit_bytes: usize # Upload body limit (default: 50 MiB)
//! └── request_timeout_secs: u64    # Per-request timeout (default: 300s)
//! ```
//!
//! All configuration can be provided via CLI arguments or environment variables.
//! Use `--help` to see all available options.
//!
//! # Example
//!
//! ```bash
//! # Configure via CLI flags
//! nvisy-server --host 127.0.0.1 --port 3000 --request-timeout-secs 60
//!
//! # Or via environment variables
//! HOST=127.0.0.1 PORT=3000 REQUEST_TIMEOUT_SECS=60 nvisy-server
//! ```

mod server;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use nvisy_server::middleware::{OpenApiConfig, RecoveryConfig, SecurityConfig};

pub use server::ServerConfig;

/// Complete CLI configuration.
///
/// Combines all configuration groups for the nvisy server:
/// - [`ServerConfig`]: Network binding and content directory
/// - Middleware settings: Body limits, timeouts, OpenAPI
#[derive(Debug, Parser)]
#[command(name = "nvisy-server", version, about = "nvisy API server")]
pub struct Cli {
    /// Server network and lifecycle configuration.
    #[command(flatten)]
    pub server: ServerConfig,

    /// Maximum body size in bytes for axum extractors (Json, Form, etc.).
    #[arg(long, env = "BODY_LIMIT_BYTES", default_value_t = 2 * 1024 * 1024)]
    pub body_limit_bytes: usize,

    /// Maximum body size in bytes for file uploads.
    #[arg(long, env = "FILE_BODY_LIMIT_BYTES", default_value_t = 50 * 1024 * 1024)]
    pub file_body_limit_bytes: usize,

    /// Per-request timeout in seconds.
    #[arg(long, env = "REQUEST_TIMEOUT_SECS", default_value_t = 300)]
    pub request_timeout_secs: u64,

    /// Allowed CORS origins (repeat for multiple). Empty means permissive.
    #[arg(long, env = "CORS_ALLOWED_ORIGINS", value_delimiter = ',')]
    pub cors_allowed_origins: Vec<String>,
}

impl Cli {
    /// Builds a [`SecurityConfig`] from the parsed CLI values.
    pub fn security_config(&self) -> SecurityConfig {
        SecurityConfig {
            body_limit_bytes: self.body_limit_bytes,
            file_body_limit_bytes: self.file_body_limit_bytes,
            cors_allowed_origins: self.cors_allowed_origins.clone(),
        }
    }

    /// Builds a [`RecoveryConfig`] from the parsed CLI values.
    pub fn recovery_config(&self) -> RecoveryConfig {
        RecoveryConfig {
            request_timeout: std::time::Duration::from_secs(self.request_timeout_secs),
        }
    }

    /// Returns the default [`OpenApiConfig`].
    pub fn open_api_config(&self) -> OpenApiConfig {
        OpenApiConfig::default()
    }

    /// Initializes tracing with environment-based filtering.
    ///
    /// Uses `RUST_LOG` if set, otherwise defaults to `info`.
    pub fn init_tracing() {
        let filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .init();
    }
}

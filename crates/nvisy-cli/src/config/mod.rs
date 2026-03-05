//! CLI configuration management.
//!
//! This module defines the complete CLI configuration hierarchy:
//!
//! ```text
//! Cli
//! ├── server: ServerConfig           # Host, port, shutdown
//! ├── service: ServiceConfig         # Data directory, engine policies
//! └── middleware: MiddlewareConfig   # Body limits, timeouts, CORS
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

mod middleware;
mod server;

use clap::Parser;
pub use middleware::MiddlewareConfig;
use nvisy_server::service::ServiceConfig;
pub use server::ServerConfig;
use tracing_subscriber::EnvFilter;

/// Complete CLI configuration.
///
/// Combines all configuration groups for the nvisy server:
/// - [`ServerConfig`]: Network binding and shutdown
/// - [`ServiceConfig`]: Data directory and engine defaults
/// - [`MiddlewareConfig`]: Body limits, timeouts, CORS, OpenAPI
#[derive(Debug, Parser)]
#[command(name = "nvisy-server", version, about = "nvisy API server")]
pub struct Cli {
    /// Server network and lifecycle configuration.
    #[command(flatten)]
    pub server: ServerConfig,

    /// Service layer configuration (registry, engine).
    #[command(flatten)]
    pub service: ServiceConfig,

    /// Middleware configuration (body limits, timeouts, CORS).
    #[command(flatten)]
    pub middleware: MiddlewareConfig,
}

impl Cli {
    /// Initializes tracing with environment-based filtering.
    ///
    /// Uses `RUST_LOG` if set, otherwise defaults to `info`.
    pub fn init_tracing() {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .init();
    }
}

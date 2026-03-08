//! CLI configuration management.
//!
//! This module defines the complete CLI configuration hierarchy:
//!
//! ```text
//! Cli (clap)
//! ├── server: ServerConfig        # Host, port, shutdown, data_dir
//! └── middleware: MiddlewareConfig # Body limits, timeouts, CORS
//!
//! FileConfig (TOML)
//! ├── server: ServerSection       # Host, port, shutdown, data_dir, observability, middleware
//! └── (flattened RuntimeConfig)   # engine, ocr, llm, stt, tts
//! ```
//!
//! The CLI parses the TOML file into [`FileConfig`], then applies clap
//! overrides from the CLI flags and environment variables.
//!
//! # Example
//!
//! ```bash
//! nvisy-server --host 127.0.0.1 --port 3000 --config Nvisy.toml
//! ```

mod file;
mod middleware;
mod server;

use std::path::PathBuf;

use clap::Parser;
use nvisy_engine::RuntimeConfig;
use tracing_subscriber::EnvFilter;

pub use file::{LogFormat, MiddlewareSection, ObservabilitySection};
pub use middleware::MiddlewareConfig;
pub use server::{ResolvedServer, ServerConfig};

/// Complete CLI configuration.
///
/// Combines all configuration groups for the nvisy server:
/// - [`ServerConfig`]: Network binding, shutdown, data directory
/// - [`MiddlewareConfig`]: Body limits, timeouts, CORS
#[derive(Debug, Parser)]
#[command(name = "nvisy-server", version, about = "nvisy API server")]
pub struct Cli {
    /// Path to a TOML configuration file (default: Nvisy.toml).
    #[arg(long, env = "NVISY_CONFIG", default_value = "Nvisy.toml")]
    pub config: PathBuf,

    /// Server network and lifecycle configuration.
    #[command(flatten)]
    pub server: ServerConfig,

    /// Middleware configuration (body limits, timeouts, CORS).
    #[command(flatten)]
    pub middleware: MiddlewareConfig,
}

impl Cli {
    /// Load the TOML file, apply CLI overrides, and return the resolved config.
    pub fn load(&self) -> anyhow::Result<(ResolvedServer, RuntimeConfig, Option<MiddlewareSection>)> {
        let toml = file::FileConfig::from_file(&self.config)?;
        let resolved = self.server.resolve(&toml.server);
        let mw_section = toml
            .server
            .as_ref()
            .and_then(|s| s.middleware.clone());
        let config = toml.inner;
        Ok((resolved, config, mw_section))
    }

    /// Initializes tracing from the resolved observability section.
    ///
    /// Uses `RUST_LOG` if set, otherwise falls back to `level` from TOML.
    pub fn init_tracing(obs: &ObservabilitySection) {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&obs.level));

        let subscriber = tracing_subscriber::fmt().with_env_filter(filter);

        match obs.format {
            LogFormat::Json => subscriber.json().init(),
            LogFormat::Text => subscriber.init(),
        }
    }
}

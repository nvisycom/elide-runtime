//! CLI configuration management.
//!
//! Parses the TOML configuration file and CLI flags, merges them using
//! CLI → TOML → default precedence, and produces fully-resolved types
//! ready for use by the server and middleware layers.
//!
//! # Architecture
//!
//! ```text
//! Cli (clap)                      CLI flags + env vars
//! └── server: ServerConfig        --host, --port, --shutdown-timeout, --data-dir
//!
//! FileConfig (Nvisy.toml)         TOML file
//! ├── server: ServerSection       [server], [server.observability], [server.middleware]
//! └── RuntimeConfig (flattened)   [engine], [ocr], [llm], [stt], [tts]
//! ```
//!
//! [`Cli::load`] reads the file, resolves server settings, and returns
//! the middleware section for the router to consume.
//!
//! # Example
//!
//! ```bash
//! nvisy --host 127.0.0.1 --port 3000 --config Nvisy.toml
//! ```

mod file;
pub mod middleware;
mod server;

use std::path::PathBuf;

use clap::Parser;
use nvisy_engine::pipeline::RuntimeConfig;
use tracing_subscriber::EnvFilter;

pub use self::file::MiddlewareSection;
pub use self::server::{ResolvedServer, ServerConfig};

/// Top-level CLI entry point.
///
/// Parses command-line arguments and loads the TOML configuration file.
/// Server network flags (`--host`, `--port`, etc.) are provided as CLI
/// arguments; all other settings come from the TOML file.
#[derive(Debug, Parser)]
#[command(name = "nvisy-server", version, about = "nvisy API server")]
pub struct Cli {
    /// Path to a TOML configuration file.
    #[arg(long, env = "NVISY_CONFIG", default_value = "Nvisy.toml")]
    pub config: PathBuf,

    /// Server network and lifecycle settings.
    #[command(flatten)]
    pub server: ServerConfig,
}

impl Cli {
    /// Loads the TOML file and resolves all configuration.
    ///
    /// Returns the resolved server settings, the runtime subsystem config,
    /// and the optional middleware section for router setup.
    pub fn load(
        &self,
    ) -> anyhow::Result<(ResolvedServer, RuntimeConfig, Option<MiddlewareSection>)> {
        let toml = file::FileConfig::from_file(&self.config)?;
        let resolved = self.server.resolve(&toml.server);
        let mw_section = toml.server.as_ref().and_then(|s| s.middleware.clone());
        let mut config = toml.inner;
        config.resolve_env();
        config
            .validate()
            .map_err(|e| anyhow::anyhow!("invalid configuration: {}", e.message))?;
        Ok((resolved, config, mw_section))
    }

    /// Initializes the global `tracing` subscriber.
    ///
    /// Uses `RUST_LOG` if set, otherwise falls back to the resolved
    /// `log_level`. Output format (JSON or text) comes from the resolved
    /// `log_format`.
    pub fn init_tracing(server: &ResolvedServer) {
        let filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&server.log_level));

        let subscriber = tracing_subscriber::fmt().with_env_filter(filter);

        match server.log_format {
            file::LogFormat::Json => subscriber.json().init(),
            file::LogFormat::Text => subscriber.init(),
        }
    }
}

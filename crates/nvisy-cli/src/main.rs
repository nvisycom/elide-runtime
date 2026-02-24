//! nvisy API server entry point.
//!
//! Parses CLI arguments, initialises tracing, constructs application state,
//! and starts the HTTP server with graceful shutdown support.

use clap::Parser;
use nvisy_core::fs::ContentRegistry;
use tracing_subscriber::EnvFilter;

mod config;
mod server;

use config::ServerConfig;

#[tokio::main]
async fn main() {
    let config = ServerConfig::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        )
        .json()
        .init();

    let content_registry = ContentRegistry::new(config.content_dir());
    let state = nvisy_server::ServiceState::new(content_registry);
    let app = server::build_router(&config, state);

    server::run(&config, app).await;
}

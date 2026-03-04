#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod config;
mod server;

use std::process;

use axum::Router;
use clap::Parser;
use nvisy_core::fs::ContentRegistry;
use nvisy_server::middleware::*;
use nvisy_server::service::ServiceState;

use crate::config::Cli;

#[tokio::main]
async fn main() {
    let Err(error) = run().await else {
        process::exit(0);
    };

    if tracing::enabled!(tracing::Level::ERROR) {
        tracing::error!(error = %error, "application terminated with error");
    } else {
        eprintln!("Error: {error:#}");
    }

    process::exit(1);
}

/// Main application entry point.
async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    Cli::init_tracing();

    // Initialize application state
    let content_registry = ContentRegistry::open(cli.server.content_dir())?;
    let state = ServiceState::new(content_registry);

    // Build and run
    let router = create_router(&cli, state);
    server::run(&cli.server, router).await
}

/// Creates the router with all middleware layers applied.
fn create_router(cli: &Cli, state: ServiceState) -> Router {
    nvisy_server::handler::routes()
        .with_open_api(&cli.open_api_config())
        .with_recovery(&cli.recovery_config())
        .with_observability()
        .with_security(&cli.security_config())
        .with_state(state)
}

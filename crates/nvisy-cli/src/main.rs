#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod config;
mod server;

use std::process;

use axum::Router;
use clap::Parser;
use nvisy_server::middleware::*;
use nvisy_server::service::ServiceState;

use crate::config::{Cli, MiddlewareSection};

const TARGET: &str = "nvisy_cli";

#[tokio::main]
async fn main() {
    let Err(error) = run().await else {
        process::exit(0);
    };

    if tracing::enabled!(tracing::Level::ERROR) {
        tracing::error!(target: TARGET, error = %error, "application terminated with error");
    } else {
        eprintln!("Error: {error:#}");
    }

    process::exit(1);
}

/// Main application entry point.
async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let (resolved, config, mw_section) = cli.load()?;
    Cli::init_tracing(&resolved);
    let state = ServiceState::new(config, resolved.data_dir.clone()).await?;
    let router = create_router(&mw_section, state);
    server::run(&resolved, router).await
}

/// Creates the router with all middleware layers applied.
fn create_router(mw_section: &Option<MiddlewareSection>, state: ServiceState) -> Router {
    nvisy_server::handler::routes()
        .with_open_api(&config::middleware::open_api_config())
        .with_recovery(&config::middleware::recovery_config(mw_section))
        .with_observability()
        .with_security(&config::middleware::security_config(mw_section))
        .with_state(state)
}

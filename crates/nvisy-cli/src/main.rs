#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod config;
mod server;

use std::process;

use axum::Router;
use clap::Parser;
use nvisy_server::middleware::{OpenApiConfig, *};
use nvisy_server::service::{ServiceRuntime, ServiceState};

use crate::config::{AppConfig, Cli, ServerConfig};

const TARGET: &str = "nvisy_cli";

#[tokio::main]
async fn main() {
    let Err(error) = run().await else {
        process::exit(0);
    };

    // `{error:#}` walks the anyhow source chain; bare Display only
    // shows the top-level message and hides the underlying cause.
    if tracing::enabled!(tracing::Level::ERROR) {
        tracing::error!(target: TARGET, error = format!("{error:#}"), "application terminated with error");
    } else {
        eprintln!("Error: {error:#}");
    }

    process::exit(1);
}

async fn run() -> anyhow::Result<()> {
    let config = Cli::parse().load()?;
    config::observability::init(&config.server.observability);
    tracing::info!(
        target: TARGET,
        binary = env!("CARGO_PKG_VERSION"),
        "starting nvisy",
    );
    let AppConfig {
        server,
        analyzer,
        llm,
    } = config;
    let runtime = ServiceRuntime::new(server.data_dir.clone(), analyzer, llm, None).await?;
    let router = create_router(&server, runtime.state());
    let outcome = server::run(&server, router).await;
    runtime.stop().await;
    outcome
}

fn create_router(server: &ServerConfig, state: ServiceState) -> Router {
    nvisy_server::handler::routes()
        .with_open_api(&OpenApiConfig::default())
        .with_recovery(&server.middleware.recovery())
        .with_observability()
        .with_security(&server.middleware.security())
        .with_state(state)
}

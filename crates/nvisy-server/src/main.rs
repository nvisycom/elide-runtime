#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod app;
mod middleware;
mod handler;
mod service;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("nvisy=info".parse()?))
        .json()
        .init();

    let config = service::ServerConfig::from_env();
    tracing::info!(host = %config.host, port = config.port, "Starting nvisy-server");

    let app = app::build_app(&config).await?;

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.host, config.port)).await?;
    tracing::info!("Listening on {}:{}", config.host, config.port);

    axum::serve(listener, app).await?;
    Ok(())
}

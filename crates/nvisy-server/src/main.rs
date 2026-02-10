mod app;
mod config;
mod middleware;
mod routes;
mod schemas;
mod service;
mod state;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("nvisy=info".parse()?))
        .json()
        .init();

    let config = config::ServerConfig::from_env();
    tracing::info!(host = %config.host, port = config.port, "Starting nvisy-server");

    let app = app::build_app(&config).await?;

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.host, config.port)).await?;
    tracing::info!("Listening on {}:{}", config.host, config.port);

    axum::serve(listener, app).await?;
    Ok(())
}

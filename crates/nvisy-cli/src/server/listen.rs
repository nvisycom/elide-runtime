//! TCP listener binding and graceful server lifecycle.

use std::path::Path;

use tokio::net::TcpListener;

use super::shutdown;
use crate::config::Cli;

/// Binds a TCP listener, serves the application, and cleans up on shutdown.
///
/// Blocks until a shutdown signal (SIGINT or SIGTERM) is received. After the
/// server stops, it removes the temporary content directory if one was created.
pub async fn run(config: &Cli, app: axum::Router) -> anyhow::Result<()> {
    let addr = config.server.socket_addr();
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(%addr, "listening");

    let shutdown = shutdown::shutdown_signal(config.server.shutdown_timeout());

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;

    cleanup_data_dir(&config.service.data_dir());
    Ok(())
}

/// Removes the temporary data directory after graceful shutdown.
fn cleanup_data_dir(path: &Path) {
    if !path.exists() {
        return;
    }
    match std::fs::remove_dir_all(path) {
        Ok(()) => tracing::info!(path = %path.display(), "data directory cleaned up"),
        Err(e) => tracing::warn!(path = %path.display(), "failed to clean up data directory: {e}"),
    }
}

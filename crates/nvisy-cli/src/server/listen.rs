//! TCP listener binding and graceful server lifecycle.

use std::fs;
use std::path::Path;

use nvisy_server::config::ServerConfig;
use tokio::net::TcpListener;

use super::shutdown;

const TARGET: &str = "nvisy_cli::server";

/// Binds a TCP listener, serves the application, and cleans up on shutdown.
///
/// Blocks until a shutdown signal (SIGINT or SIGTERM) is received. After the
/// server stops, it removes the data directory if one was created.
pub async fn run(server: &ServerConfig, app: axum::Router) -> anyhow::Result<()> {
    let addr = server.socket_addr();
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(target: TARGET, %addr, "listening");

    let shutdown = shutdown::shutdown_signal(server.shutdown_timeout);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;

    cleanup_data_dir(&server.data_dir);
    Ok(())
}

/// Removes the data directory after graceful shutdown.
fn cleanup_data_dir(path: &Path) {
    if !path.exists() {
        return;
    }
    match fs::remove_dir_all(path) {
        Ok(()) => {
            tracing::info!(target: TARGET, path = %path.display(), "data directory cleaned up")
        }
        Err(e) => {
            tracing::warn!(target: TARGET, path = %path.display(), "failed to clean up data directory: {e}")
        }
    }
}

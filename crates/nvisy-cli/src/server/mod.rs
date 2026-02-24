//! Server lifecycle: router construction, TCP listener, and graceful shutdown.

use std::path::Path;

use tokio::net::TcpListener;

use nvisy_server::middleware::{
    RouterObservabilityExt, RouterOpenApiExt, RouterRecoveryExt, RouterSecurityExt,
};
use nvisy_server::ServiceState;

use crate::config::ServerConfig;

/// Builds the application router with all middleware layers applied.
pub fn build_router(config: &ServerConfig, state: ServiceState) -> axum::Router {
    nvisy_server::routes()
        .with_open_api(&config.open_api_config())
        .with_recovery(&config.recovery_config())
        .with_observability()
        .with_security(&config.security_config())
        .with_state(state)
}

/// Binds a TCP listener, serves the application, and cleans up on shutdown.
///
/// Blocks until a shutdown signal (SIGINT or SIGTERM) is received. After the
/// server stops, it removes the temporary content directory if one was created.
pub async fn run(config: &ServerConfig, app: axum::Router) {
    let addr = config.socket_addr();

    let listener = TcpListener::bind(addr).await.unwrap_or_else(|e| {
        panic!("failed to bind to {addr}: {e}");
    });

    tracing::info!(%addr, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap_or_else(|e| {
            panic!("server error: {e}");
        });

    cleanup_content_dir(&config.content_dir());
}

/// Waits for SIGINT (Ctrl+C) or SIGTERM to initiate graceful shutdown.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!("shutdown signal received");
}

/// Removes the temporary content directory after graceful shutdown.
fn cleanup_content_dir(path: &Path) {
    if !path.exists() {
        return;
    }
    match std::fs::remove_dir_all(path) {
        Ok(()) => tracing::info!(path = %path.display(), "content directory cleaned up"),
        Err(e) => tracing::warn!(path = %path.display(), "failed to clean up content directory: {e}"),
    }
}

use std::sync::Arc;

use nvisy_core::fs::ContentRegistry;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

mod handler;
mod middleware;
mod service;

use service::{ServiceState, StubEngine, build_router};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    let addr = format!("{host}:{port}");

    let state = ServiceState {
        engine: Arc::new(StubEngine),
        content_registry: ContentRegistry::new(
            std::env::temp_dir().join("nvisy-server-content"),
        ),
    };

    let app = build_router(state);

    let listener = TcpListener::bind(&addr).await.unwrap_or_else(|e| {
        panic!("failed to bind to {addr}: {e}");
    });

    tracing::info!("listening on {addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap_or_else(|e| {
            panic!("server error: {e}");
        });
}

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

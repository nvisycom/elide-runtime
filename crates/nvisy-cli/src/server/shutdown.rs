//! Graceful shutdown signal handling.

use std::time::Duration;

use tokio::signal::ctrl_c;
#[cfg(unix)]
use tokio::signal::unix;

use super::TRACING_TARGET_SHUTDOWN;

/// Waits for a shutdown signal (SIGTERM or SIGINT/Ctrl+C).
///
/// Listens for OS termination signals and returns when one is received.
/// The `shutdown_timeout` is logged to inform operators how long cleanup
/// will wait before the process is forcefully terminated.
pub async fn shutdown_signal(shutdown_timeout: Duration) {
    let ctrl_c = async {
        if let Err(e) = ctrl_c().await {
            tracing::error!(
                target: TRACING_TARGET_SHUTDOWN,
                error = %e,
                "failed to install Ctrl+C handler"
            );
        } else {
            tracing::info!(
                target: TRACING_TARGET_SHUTDOWN,
                "received Ctrl+C signal, initiating graceful shutdown"
            );
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match unix::signal(unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
                tracing::info!(
                    target: TRACING_TARGET_SHUTDOWN,
                    "received SIGTERM signal, initiating graceful shutdown"
                );
            }
            Err(e) => {
                tracing::error!(
                    target: TRACING_TARGET_SHUTDOWN,
                    error = %e,
                    "failed to install SIGTERM handler"
                );
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!(
        target: TRACING_TARGET_SHUTDOWN,
        timeout_secs = shutdown_timeout.as_secs(),
        "graceful shutdown initiated"
    );
}

//! Server lifecycle: TCP listener and graceful shutdown.

mod listen;
mod shutdown;

/// Tracing target for shutdown events.
pub const TRACING_TARGET_SHUTDOWN: &str = "nvisy_cli::server::shutdown";

pub use listen::run;

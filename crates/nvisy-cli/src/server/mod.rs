//! Server lifecycle: TCP listener and graceful shutdown.

mod listen;
mod shutdown;

pub use listen::run;

//! CLI-side observability init.
//!
//! The config *type* ([`ObservabilityConfig`]) lives in
//! `nvisy-server`; this module owns the runtime action of
//! wiring `tracing-subscriber` from that config, which drags in
//! the subscriber deps and belongs with the binary.

use nvisy_server::config::{LogFormat, ObservabilityConfig};
use tracing_subscriber::EnvFilter;

/// Initialise the global `tracing` subscriber from `config`.
///
/// `RUST_LOG` takes precedence over
/// [`ObservabilityConfig::level`] when set.
pub fn init(config: &ObservabilityConfig) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.level));
    let subscriber = tracing_subscriber::fmt().with_env_filter(filter);
    match config.format {
        LogFormat::Json => subscriber.json().init(),
        LogFormat::Text => subscriber.init(),
    }
}

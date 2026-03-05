//! Application state and dependency injection.
//!
//! [`ServiceState`] holds shared dependencies (engine, registry) and is
//! threaded through every handler via Axum's `State` extractor. Individual
//! handlers extract only the dependency they need (e.g. `State<Registry>`)
//! rather than the full state.

mod config;

pub use config::ServiceConfig;
use nvisy_engine::pipeline::DefaultEngine;
use nvisy_registry::Registry;

/// Shared application state threaded through all handlers.
#[must_use = "state does nothing unless you use it"]
#[derive(Clone)]
pub struct ServiceState {
    engine: DefaultEngine,
    registry: Registry,
}

impl ServiceState {
    /// Creates a new service state from a [`ServiceConfig`].
    ///
    /// # Errors
    ///
    /// Returns an error if the registry database cannot be opened.
    pub fn new(config: &ServiceConfig) -> nvisy_core::Result<Self> {
        let registry = Registry::open(config.data_dir())?;

        let mut engine = DefaultEngine::new();
        if let Some(retry) = config.retry_policy() {
            engine = engine.with_retry(retry);
        }
        if let Some(timeout) = config.timeout_policy() {
            engine = engine.with_timeout(timeout);
        }

        Ok(Self { engine, registry })
    }
}

macro_rules! impl_di {
    ($($f:ident: $t:ty),+ $(,)?) => {$(
        impl axum::extract::FromRef<ServiceState> for $t {
            fn from_ref(state: &ServiceState) -> Self {
                state.$f.clone()
            }
        }
    )+};
}

impl_di!(
    engine: DefaultEngine,
    registry: Registry,
);

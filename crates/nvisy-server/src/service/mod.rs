//! Application state and dependency injection.
//!
//! [`ServiceState`] holds shared dependencies (engine, registry) and is
//! threaded through every handler via Axum's `State` extractor. Individual
//! handlers extract only the dependency they need (e.g. `State<Registry>`)
//! rather than the full state.

use std::path::PathBuf;

use nvisy_engine::{DefaultEngine, RuntimeConfig};
use nvisy_http::HttpClient;
use nvisy_registry::Registry;

/// Shared application state threaded through all handlers.
#[must_use = "state does nothing unless you use it"]
#[derive(Clone)]
pub struct ServiceState {
    engine: DefaultEngine,
    registry: Registry,
}

impl ServiceState {
    /// Creates a new service state from a resolved [`RuntimeConfig`] and data directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry database cannot be opened.
    pub fn new(config: RuntimeConfig, data_dir: PathBuf) -> nvisy_core::Result<Self> {
        let registry = Registry::open(data_dir)?;

        let http_config = config
            .engine
            .as_ref()
            .and_then(|e| e.http.clone())
            .unwrap_or_default();
        let http_client = HttpClient::new(&http_config);

        let mut engine = DefaultEngine::new()
            .with_config(config)
            .with_http_client(http_client);
        if let Some(retry) = engine
            .config()
            .engine
            .as_ref()
            .and_then(|e| e.retry.clone())
        {
            engine = engine.with_retry(retry);
        }
        if let Some(timeout) = engine
            .config()
            .engine
            .as_ref()
            .and_then(|e| e.timeout.clone())
        {
            engine = engine.with_timeout(timeout);
        }

        Ok(Self { engine, registry })
    }

    /// Returns the data directory path from the registry.
    pub fn data_dir(&self) -> &std::path::Path {
        self.registry.base_dir()
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

//! Application state and dependency injection.
//!
//! [`ServiceState`] holds the [`DefaultEngine`] which owns all shared
//! dependencies (registry, HTTP client, policies). Individual handlers
//! extract the dependency they need (e.g. `State<Registry>`) via
//! `FromRef` implementations that pull from the engine.

use std::path::{Path, PathBuf};

use nvisy_engine::pipeline::{DefaultEngine, RuntimeConfig};
use nvisy_http::HttpClient;
use nvisy_registry::Registry;

/// Shared application state threaded through all handlers.
#[must_use = "state does nothing unless you use it"]
#[derive(Clone)]
pub struct ServiceState {
    engine: DefaultEngine,
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

        let engine = DefaultEngine::new(registry)
            .with_config(config)
            .with_http_client(http_client);

        Ok(Self { engine })
    }

    /// Returns the data directory path from the registry.
    pub fn data_dir(&self) -> &Path {
        self.engine.registry().base_dir()
    }
}

macro_rules! impl_di {
    ($($extract:expr => $t:ty),+ $(,)?) => {$(
        impl axum::extract::FromRef<ServiceState> for $t {
            fn from_ref(state: &ServiceState) -> Self {
                $extract(state)
            }
        }
    )+};
}

impl_di!(
    |s: &ServiceState| s.engine.clone() => DefaultEngine,
    |s: &ServiceState| s.engine.registry().clone() => Registry,
);

//! Application state and dependency injection.
//!
//! [`ServiceState`] holds the [`Engine`] which owns all shared
//! dependencies (registry, HTTP client, policies). Individual handlers
//! extract the engine via a `FromRef` implementation.

use std::path::{Path, PathBuf};

use nvisy_engine::ingestion::registry::Registry;
use nvisy_engine::pipeline::{Engine, RuntimeConfig};

/// Shared application state threaded through all handlers.
#[must_use = "state does nothing unless you use it"]
#[derive(Clone)]
pub struct ServiceState {
    engine: Engine,
    registry: Registry,
}

impl ServiceState {
    /// Creates a new service state from a resolved [`RuntimeConfig`] and data directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry database cannot be opened.
    pub fn new(config: RuntimeConfig, data_dir: PathBuf) -> nvisy_core::Result<Self> {
        let engine = Engine::open(data_dir, config)?;
        let registry = engine.registry().clone();
        Ok(Self { engine, registry })
    }

    /// Returns the data directory path.
    pub fn data_dir(&self) -> &Path {
        self.engine.data_dir()
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
    |s: &ServiceState| s.engine.clone() => Engine,
    |s: &ServiceState| s.registry.clone() => Registry,
);

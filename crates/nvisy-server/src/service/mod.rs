//! Application state and dependency injection.
//!
//! [`ServiceState`] holds the [`Engine`] which owns all shared
//! dependencies (registry, codec set, recognizer / extractor
//! registries). Individual handlers extract the engine via a
//! `FromRef` implementation.

use std::path::{Path, PathBuf};

use nvisy_core::Result;
use nvisy_engine::pipeline::{Engine, RuntimeConfig};
use nvisy_engine::registry::Registry;

/// Shared application state threaded through all handlers.
#[must_use = "state does nothing unless you use it"]
#[derive(Clone)]
pub struct ServiceState {
    engine: Engine,
}

impl ServiceState {
    /// Creates a new service state from a resolved [`RuntimeConfig`] and data directory.
    ///
    /// Async because engine construction may download model
    /// artifacts on first use.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry database cannot be opened.
    pub async fn new(config: RuntimeConfig, data_dir: PathBuf) -> Result<Self> {
        let engine = Engine::open(data_dir, config).await?;
        Ok(Self { engine })
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
    |s: &ServiceState| s.engine.registry().clone() => Registry,
);

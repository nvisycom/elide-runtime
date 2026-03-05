//! Application state and dependency injection.
//!
//! [`ServiceState`] holds shared dependencies (engine, registry) and is
//! threaded through every handler via Axum's `State` extractor. Fields are
//! private; use the provided accessor methods.

use nvisy_engine::pipeline::DefaultEngine;
use nvisy_registry::Registry;

/// Shared application state threaded through all handlers.
#[must_use = "state does nothing unless you use it"]
#[derive(Clone)]
pub struct ServiceState {
    default_engine: DefaultEngine,
    registry: Registry,
}

impl ServiceState {
    /// Creates a new service state with the given registry.
    pub fn new(registry: Registry) -> Self {
        Self {
            default_engine: DefaultEngine::new(),
            registry,
        }
    }

    /// Returns a reference to the pipeline engine.
    pub fn engine(&self) -> &DefaultEngine {
        &self.default_engine
    }

    /// Returns a reference to the registry.
    pub fn registry(&self) -> &Registry {
        &self.registry
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
    default_engine: DefaultEngine,
    registry: Registry,
);

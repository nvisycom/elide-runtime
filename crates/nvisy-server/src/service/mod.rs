//! Application state and dependency injection.
//!
//! [`ServiceState`] holds shared dependencies (engine, content registry) and is
//! threaded through every handler via Axum's `State` extractor. Fields are
//! private; use the provided accessor methods.

use nvisy_core::fs::ContentRegistry;
use nvisy_engine::pipeline::DefaultEngine;

/// Shared application state threaded through all handlers.
#[must_use = "state does nothing unless you use it"]
#[derive(Clone)]
pub struct ServiceState {
    default_engine: DefaultEngine,
    content_registry: ContentRegistry,
}

impl ServiceState {
    /// Creates a new service state with the given content registry.
    pub fn new(content_registry: ContentRegistry) -> Self {
        Self {
            default_engine: DefaultEngine,
            content_registry,
        }
    }

    /// Returns a reference to the pipeline engine.
    pub fn engine(&self) -> &DefaultEngine {
        &self.default_engine
    }

    /// Returns a reference to the content registry.
    pub fn content_registry(&self) -> &ContentRegistry {
        &self.content_registry
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
    content_registry: ContentRegistry,
);

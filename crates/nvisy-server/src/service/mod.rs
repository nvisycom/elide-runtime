//! Application state and dependency injection.
//!
//! [`ServiceState`] holds shared dependencies (engine, content registry) and is
//! threaded through every handler via Axum's `State` extractor. Fields are
//! private; use the provided accessor methods.

use std::sync::Arc;

use nvisy_core::fs::ContentRegistry;
use nvisy_core::{Error, ErrorKind};
use nvisy_engine::engine::{Engine, EngineInput, EngineOutput};

/// Shared application state threaded through all handlers.
///
/// The engine is stored behind [`Arc`] with a manual [`Clone`] impl because
/// [`Engine`] uses RPITIT and is not dyn-compatible.
pub struct ServiceState {
    engine: Arc<StubEngine>,
    content_registry: ContentRegistry,
}

impl ServiceState {
    /// Creates a new service state with the given content registry.
    ///
    /// Wires in the [`StubEngine`] until a real implementation is configured.
    pub fn new(content_registry: ContentRegistry) -> Self {
        Self {
            engine: Arc::new(StubEngine),
            content_registry,
        }
    }

    /// Returns a reference to the pipeline engine.
    pub fn engine(&self) -> &StubEngine {
        &self.engine
    }

    /// Returns a reference to the content registry.
    pub fn content_registry(&self) -> &ContentRegistry {
        &self.content_registry
    }
}

impl Clone for ServiceState {
    fn clone(&self) -> Self {
        Self {
            engine: Arc::clone(&self.engine),
            content_registry: self.content_registry.clone(),
        }
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
    content_registry: ContentRegistry,
);

/// Placeholder engine that rejects all requests.
///
/// Wired in at startup until a real implementation is configured.
pub struct StubEngine;

impl Engine for StubEngine {
    async fn run(&self, _input: EngineInput) -> Result<EngineOutput, Error> {
        Err(Error::new(ErrorKind::Runtime, "no engine configured"))
    }
}

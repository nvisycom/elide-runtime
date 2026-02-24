use std::sync::Arc;

use aide::openapi::{Info, OpenApi};
use axum::Extension;
use nvisy_core::fs::ContentRegistry;
use nvisy_core::{Error, ErrorKind};
use nvisy_engine::engine::{Engine, EngineInput, EngineOutput};

use crate::handler;
use crate::middleware;

/// Shared application state threaded through all handlers.
///
/// The engine is stored behind `Arc` with a manual `Clone` impl
/// since `Engine` uses RPITIT and is not dyn-compatible.
pub struct ServiceState {
    pub engine: Arc<StubEngine>,
    pub content_registry: ContentRegistry,
}

impl Clone for ServiceState {
    fn clone(&self) -> Self {
        Self {
            engine: Arc::clone(&self.engine),
            content_registry: self.content_registry.clone(),
        }
    }
}

/// Placeholder engine that rejects all requests.
///
/// Wired in at startup until a real implementation is configured.
pub struct StubEngine;

impl Engine for StubEngine {
    async fn run(&self, _input: EngineInput) -> Result<EngineOutput, Error> {
        Err(Error::new(ErrorKind::Runtime, "no engine configured"))
    }
}

/// Build the application router with OpenAPI documentation.
pub fn build_router(state: ServiceState) -> axum::Router {
    let (set_request_id, propagate_request_id, trace, cors, timeout) =
        middleware::middleware_stack();

    let app = handler::routes().with_state(state);

    let mut api = OpenApi {
        info: Info {
            title: "nvisy API".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: Some("REST API for the nvisy redaction engine.".to_string()),
            ..Info::default()
        },
        ..OpenApi::default()
    };

    app.finish_api(&mut api)
        .layer(Extension(api))
        .layer(trace)
        .layer(cors)
        .layer(timeout)
        .layer(set_request_id)
        .layer(propagate_request_id)
}

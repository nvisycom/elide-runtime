//! HTTP application bootstrap and route composition.
//!
//! The [`build_app`] function wires together all Axum routers, middleware
//! (CORS, tracing), and shared application state into a single [`Router`].

use axum::Router;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::handler;
use crate::service::{AuditStore, AppState, PolicyStore, ServerConfig};
use nvisy_engine::runs::RunManager;

/// Build a fully configured Axum [`Router`] with all handlers and middleware.
///
/// This constructs the shared [`AppState`], applies CORS and HTTP tracing
/// layers, and merges the health, graphs, redact, policies, audit, and
/// Scalar API-docs routes.
pub async fn build_app(_config: &ServerConfig) -> anyhow::Result<Router> {
    let state = AppState {
        run_manager: Arc::new(RunManager::new()),
        policy_store: Arc::new(PolicyStore::new()),
        audit_store: Arc::new(AuditStore::new()),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(handler::health::router())
        .merge(handler::graphs::router())
        .merge(handler::redact::router())
        .merge(handler::policies::router())
        .merge(handler::audit::router())
        .merge(Scalar::with_url("/scalar", handler::ApiDoc::openapi()))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    Ok(app)
}

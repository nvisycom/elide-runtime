use axum::Router;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::config::ServerConfig;
use crate::routes;
use crate::service::engine_factory;
use crate::service::audit_store::AuditStore;
use crate::service::policy_store::PolicyStore;
use crate::state::AppState;
use nvisy_engine::runs::RunManager;

/// Build a fully configured Axum application.
pub async fn build_app(_config: &ServerConfig) -> anyhow::Result<Router> {
    let registry = engine_factory::create_registry()?;

    let state = AppState {
        registry: Arc::new(registry),
        run_manager: Arc::new(RunManager::new()),
        policy_store: Arc::new(PolicyStore::new()),
        audit_store: Arc::new(AuditStore::new()),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(routes::health::router())
        .merge(routes::graphs::router())
        .merge(routes::redact::router())
        .merge(routes::policies::router())
        .merge(routes::audit::router())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    Ok(app)
}

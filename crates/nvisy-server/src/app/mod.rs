use axum::Router;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::handler;
use crate::service::engine_factory;
use crate::service::{AuditStore, AppState, PolicyStore, ServerConfig};
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
        .merge(handler::health::router())
        .merge(handler::graphs::router())
        .merge(handler::redact::router())
        .merge(handler::policies::router())
        .merge(handler::audit::router())
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", handler::ApiDoc::openapi()))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    Ok(app)
}

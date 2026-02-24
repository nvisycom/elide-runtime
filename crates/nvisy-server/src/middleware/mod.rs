//! HTTP middleware stack: request IDs, tracing, CORS, timeouts,
//! body limits, and response compression.

use std::time::Duration;

use axum::http::StatusCode;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{self, TraceLayer};
use tracing::Level;

const DEFAULT_TIMEOUT_SECS: u64 = 300;
const REQUEST_ID_HEADER: &str = "x-request-id";
/// Default request body limit (50 MiB).
const DEFAULT_BODY_LIMIT: usize = 50 * 1024 * 1024;

/// Build the shared middleware stack.
pub fn middleware_stack() -> (
    SetRequestIdLayer<MakeRequestUuid>,
    PropagateRequestIdLayer,
    TraceLayer<tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>>,
    CorsLayer,
    TimeoutLayer,
    RequestBodyLimitLayer,
    CompressionLayer,
) {
    let timeout_secs: u64 = std::env::var("REQUEST_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECS);

    let body_limit: usize = std::env::var("REQUEST_BODY_LIMIT_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_BODY_LIMIT);

    let header = axum::http::HeaderName::from_static(REQUEST_ID_HEADER);

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
        .on_response(trace::DefaultOnResponse::new().level(Level::INFO));

    (
        SetRequestIdLayer::new(header.clone(), MakeRequestUuid),
        PropagateRequestIdLayer::new(header),
        trace_layer,
        CorsLayer::permissive(),
        TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            Duration::from_secs(timeout_secs),
        ),
        RequestBodyLimitLayer::new(body_limit),
        CompressionLayer::new(),
    )
}

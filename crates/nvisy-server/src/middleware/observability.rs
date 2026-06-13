//! Observability middleware for tracing and request correlation.
//!
//! Combines request ID generation, structured HTTP tracing, sensitive header
//! redaction, and request/response logging into a single middleware stack.
//!
//! Each inbound request receives a unique `x-request-id` UUID that is:
//! - Included in the tracing span for all downstream log events.
//! - Propagated back to the client on the response.
//!
//! Sensitive headers (`Authorization`, `Cookie`) are redacted from trace
//! output to prevent credential leakage.

use axum::Router;
use axum::http::header;
use tower_http::classify::SharedClassifier;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::trace::{self, TraceLayer};

/// Header name used for request correlation.
const REQUEST_ID_HEADER: &str = "x-request-id";

/// Extension trait for [`Router`] to add observability middleware.
///
/// Layers the full observability stack in the correct order:
///
/// 1. **Propagate request ID** on responses (outermost).
/// 2. **Redact sensitive headers** from trace output.
/// 3. **HTTP tracing** with method, URI, and status code spans.
/// 4. **Set request ID** on inbound requests (innermost).
pub trait RouterObservabilityExt<S> {
    /// Layers request ID generation, structured tracing, response ID
    /// propagation, and sensitive header redaction.
    fn with_observability(self) -> Self;
}

impl<S> RouterObservabilityExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_observability(self) -> Self {
        self.layer(PropagateRequestIdLayer::new(
            header::HeaderName::from_static(REQUEST_ID_HEADER),
        ))
        .layer(SetSensitiveRequestHeadersLayer::new([
            header::AUTHORIZATION,
            header::COOKIE,
        ]))
        .layer(trace_layer())
        .layer(SetRequestIdLayer::new(
            header::HeaderName::from_static(REQUEST_ID_HEADER),
            MakeRequestUuid,
        ))
    }
}

/// Builds the [`TraceLayer`] with structured spans and callbacks.
fn trace_layer() -> TraceLayer<SharedClassifier<tower_http::classify::ServerErrorsAsFailures>> {
    TraceLayer::new_for_http()
        .make_span_with(
            trace::DefaultMakeSpan::new()
                .level(tracing::Level::INFO)
                .include_headers(false),
        )
        .on_request(trace::DefaultOnRequest::new().level(tracing::Level::DEBUG))
        .on_response(
            trace::DefaultOnResponse::new()
                .level(tracing::Level::INFO)
                .include_headers(false),
        )
        .on_failure(trace::DefaultOnFailure::new().level(tracing::Level::ERROR))
}

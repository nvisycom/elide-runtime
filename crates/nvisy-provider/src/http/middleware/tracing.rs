//! OpenTelemetry tracing middleware.

use reqwest_tracing::{DefaultSpanBackend, TracingMiddleware};

/// Create the default tracing middleware layer.
pub fn layer() -> TracingMiddleware<DefaultSpanBackend> {
    TracingMiddleware::default()
}

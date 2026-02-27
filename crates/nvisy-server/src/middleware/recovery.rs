//! Recovery middleware for handling errors, panics, and timeouts.
//!
//! Provides middleware for recovering from various error conditions in the
//! request/response lifecycle, ensuring graceful degradation and proper
//! error responses to clients.
//!
//! # Usage
//!
//! ```rust,ignore
//! use nvisy_server::middleware::recovery::{RecoveryConfig, RouterRecoveryExt};
//!
//! let app = Router::new()
//!     .with_recovery(&RecoveryConfig::default());
//! ```

use std::any::Any;
use std::future::ready;
use std::time::Duration;

use axum::Router;
use axum::error_handling::HandleErrorLayer;
use axum::response::{IntoResponse, Response};
use futures::future::{BoxFuture, FutureExt};
use serde::{Deserialize, Serialize};
use tower::ServiceBuilder;
use tower::timeout::TimeoutLayer;
use tower_http::catch_panic::CatchPanicLayer;

use crate::handler::ServerError;

/// Tracing target for error recovery.
const TRACING_TARGET_ERROR: &str = "nvisy_server::recovery::error";

/// Tracing target for panic recovery.
const TRACING_TARGET_PANIC: &str = "nvisy_server::recovery::panic";

type ResponseFut = BoxFuture<'static, Response>;
type Panic = Box<dyn Any + Send + 'static>;

/// Configuration for recovery middleware behavior.
///
/// Controls how the recovery middleware handles various error conditions
/// including timeouts and panic recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConfig {
    /// Maximum duration in seconds to wait for a request to complete
    /// before timing out. Requests exceeding this duration receive a
    /// 500 response with a timeout message.
    pub request_timeout: u64,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            request_timeout: 300,
        }
    }
}

impl RecoveryConfig {
    /// Returns the request timeout as a [`Duration`].
    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout)
    }
}

/// Extension trait for [`Router`] to apply recovery middleware.
///
/// Adds error recovery capabilities to the router, protecting against
/// panics in handlers and enforcing request timeouts.
pub trait RouterRecoveryExt<S> {
    /// Layers recovery middleware with the provided configuration.
    ///
    /// The middleware stack handles request timeouts, panics in handlers,
    /// and Tower service errors, converting them to appropriate HTTP
    /// error responses.
    fn with_recovery(self, config: &RecoveryConfig) -> Self;
}

impl<S> RouterRecoveryExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_recovery(self, config: &RecoveryConfig) -> Self {
        let middlewares = ServiceBuilder::new()
            .layer(HandleErrorLayer::new(handle_error))
            .layer(CatchPanicLayer::custom(catch_panic))
            .layer(TimeoutLayer::new(config.request_timeout()));

        self.layer(middlewares)
    }
}

fn handle_error(err: tower::BoxError) -> ResponseFut {
    use tower::timeout::error::Elapsed;

    if err.downcast_ref::<Elapsed>().is_some() {
        tracing::error!(
            target: TRACING_TARGET_ERROR,
            error = %err,
            "request timeout exceeded",
        );

        let error = nvisy_core::Error::new(nvisy_core::ErrorKind::Timeout, "request timeout");
        return ready(ServerError::from(error).into_response()).boxed();
    }

    tracing::error!(
        target: TRACING_TARGET_ERROR,
        error = %err,
        "unhandled middleware error",
    );

    let error = nvisy_core::Error::new(
        nvisy_core::ErrorKind::InternalError,
        format!("internal error: {err}"),
    );
    ready(ServerError::from(error).into_response()).boxed()
}

fn catch_panic(err: Panic) -> Response {
    let message = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap_or("unknown panic");

    tracing::error!(
        target: TRACING_TARGET_PANIC,
        message = %message,
        "service panic",
    );

    let error = nvisy_core::Error::new(nvisy_core::ErrorKind::InternalError, "service panic");
    ServerError::from(error).into_response()
}

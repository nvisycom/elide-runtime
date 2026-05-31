//! Engine-level configuration: networking and resource limits.

use std::num::NonZeroUsize;
use std::time::Duration;

use nvisy_core::http::HttpConfig;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Hard limits on pipeline resource consumption.
///
/// Deployment-side caps read once during [`Engine::run`] and not
/// adjustable per-request.
///
/// [`Engine::run`]: super::super::Engine::run
#[derive(Debug, Clone, Copy, Default, Validate, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum number of documents processed in parallel via a
    /// shared [`Semaphore`]. Server-wide; not overridable
    /// per-request. `None` means unbounded.
    ///
    /// [`Semaphore`]: tokio::sync::Semaphore
    #[serde(default)]
    pub concurrency: Option<NonZeroUsize>,

    /// Hard ceiling on total pipeline run duration.
    ///
    /// On expiry, the run-level cancellation token fires and the run
    /// is marked as timed out. `None` means no run-level timeout —
    /// rely on external supervision (k8s liveness, etc.) instead.
    ///
    /// Parses from human-friendly strings via `humantime_serde`:
    /// `"60s"`, `"5m"`, `"1h30m"`.
    #[serde(
        default,
        with = "humantime_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub run_timeout: Option<Duration>,
}

/// Engine-level configuration: networking + resource limits.
///
/// All settings are deployment-side — set once in `Nvisy.toml` by
/// the operator. Per-request overrides apply only to fields
/// explicitly noted as overridable.
#[derive(Debug, Clone, Default, Validate, Serialize, Deserialize)]
pub struct EngineConfig {
    /// Shared HTTP client configuration for all downstream API calls.
    ///
    /// Applies to OCR providers, LLM agents, STT services, and any
    /// other external HTTP dependencies. Controls timeouts, retries,
    /// and connection pooling.
    pub http: Option<HttpConfig>,

    /// Run-level resource limits (concurrency cap + timeout).
    ///
    /// Nested under `[engine.limits]` in TOML.
    #[validate(nested)]
    #[serde(default)]
    pub limits: ResourceLimits,
}

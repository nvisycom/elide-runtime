//! Engine-level configuration: networking, resource limits, concurrency.

use std::time::Duration;

use nvisy_http::HttpConfig;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::pipeline::ConcurrencyPolicy;

/// Hard limits on pipeline resource consumption.
///
/// Deployment-side caps read once during [`Engine::run`] and not
/// adjustable per-request.
///
/// [`Engine::run`]: super::super::Engine::run
#[derive(Debug, Clone, Copy, Default, Validate, Serialize, Deserialize)]
pub struct ResourceLimits {
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

    /// Maximum number of content IDs per import.
    ///
    /// Caps the fan-out of a single import operation. `None` means no
    /// limit (not yet enforced — reserved for future use).
    #[serde(default)]
    pub max_content_ids_per_import: Option<usize>,

    /// Maximum envelope payload size in bytes.
    ///
    /// Documents exceeding this threshold are rejected at import time.
    /// `None` means no limit (not yet enforced — reserved for future
    /// use).
    #[serde(default)]
    pub max_envelope_size_bytes: Option<u64>,
}

/// Cache tuning parameters.
///
/// Controls resource cache behavior for contexts and policies held in
/// the [`Registry`]. Not yet enforced — reserved for future use.
///
/// [`Registry`]: crate::registry::Registry
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Maximum number of entries to keep in each resource cache.
    ///
    /// When exceeded, entries are evicted in LRU order. `None` means
    /// no limit (current default behavior).
    #[serde(default)]
    pub max_entries: Option<usize>,
}

/// Engine-level configuration: concurrency, networking, resource
/// limits, and cache tuning.
///
/// All settings are deployment-side — set once in `Nvisy.toml` by
/// the operator. Per-request overrides apply only to fields
/// explicitly noted as overridable.
#[derive(Debug, Clone, Default, Validate, Serialize, Deserialize)]
pub struct EngineSection {
    /// Concurrency limit for parallel document execution.
    ///
    /// Caps the number of documents processed in parallel via
    /// a [`tokio::sync::Semaphore`]. Server-wide; not overridable
    /// per-request.
    #[serde(default)]
    pub concurrency: Option<ConcurrencyPolicy>,

    /// Shared HTTP client configuration for all downstream API calls.
    ///
    /// Applies to OCR providers, LLM agents, STT/TTS services, and any
    /// other external HTTP dependencies. Controls timeouts, retries,
    /// and connection pooling.
    pub http: Option<HttpConfig>,

    /// Run-level resource limits.
    ///
    /// Nested under `[engine.limits]` in TOML.
    #[validate(nested)]
    #[serde(default)]
    pub limits: ResourceLimits,

    /// Cache tuning parameters for context and policy caches.
    ///
    /// Nested under `[engine.cache]` in TOML. Not yet enforced —
    /// reserved for future use.
    #[serde(default)]
    pub cache: Option<CacheConfig>,
}

//! Engine-level execution policies, networking, and resource limits.

use nvisy_http::HttpConfig;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::pipeline::{ConcurrencyPolicy, TimeoutPolicy};

/// Hard limits on pipeline resource consumption.
///
/// These values cap the overall duration, buffer sizes, and graph
/// complexity for a single pipeline run. They are read once during
/// [`Engine::run`] and cannot be changed mid-run.
///
/// [`Engine::run`]: super::super::Engine::run
#[derive(Debug, Clone, Copy, Default, Validate, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Hard ceiling on total pipeline run duration, in milliseconds.
    ///
    /// If a run exceeds this limit, the cancellation token is triggered
    /// and the run is marked as timed out. Individual node timeouts
    /// (via [`TimeoutPolicy`]) are independent of this limit.
    /// `None` means no run-level timeout.
    #[serde(default)]
    pub run_timeout_ms: Option<u64>,

    /// Reserved; previously capped the size of the execution graph.
    /// Retained for config-schema compatibility — not enforced.
    #[serde(default)]
    pub max_nodes: Option<usize>,

    /// Maximum number of content IDs per import node.
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

/// Engine-level execution policies, networking, and resource limits.
///
/// Controls default behavior for all pipeline runs unless overridden
/// by per-request configuration.
///
/// # Field groups
///
/// **Execution policies** — applied to phases that lack their own:
/// - [`timeout`] — per-phase wall-clock deadline.
/// - [`concurrency`] — limits parallel document execution.
///
/// **Networking:**
/// - [`http`] — shared HTTP client settings (timeouts, retries,
///   connection pooling) for all downstream API calls.
///
/// **Resource limits:**
/// - [`limits`] — run timeout, channel buffer size, graph complexity.
///
/// **Cache tuning:**
/// - [`cache`] — resource cache size limits (reserved for future use).
///
/// [`timeout`]: Self::timeout
/// [`concurrency`]: Self::concurrency
/// [`http`]: Self::http
/// [`limits`]: Self::limits
/// [`cache`]: Self::cache
#[derive(Debug, Clone, Default, Validate, Serialize, Deserialize)]
pub struct EngineSection {
    /// Default timeout policy applied to phases without an explicit
    /// per-phase policy on the pipeline input.
    ///
    /// Sets a per-phase wall-clock deadline and behavior on expiry
    /// (fail or skip).
    #[validate(nested)]
    pub timeout: Option<TimeoutPolicy>,

    /// Default concurrency limit for parallel document execution.
    ///
    /// Caps the number of documents processed in parallel via
    /// a [`tokio::sync::Semaphore`]. Overridden by
    /// `EngineInput::concurrency` per-run.
    #[serde(default)]
    pub concurrency: Option<ConcurrencyPolicy>,

    /// Shared HTTP client configuration for all downstream API calls.
    ///
    /// Applies to OCR providers, LLM agents, STT/TTS services, and any
    /// other external HTTP dependencies. Controls timeouts, retries,
    /// and connection pooling.
    pub http: Option<HttpConfig>,

    /// Run-level resource limits (timeout, channel buffer size, graph
    /// complexity caps).
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

impl EngineSection {
    /// Returns the configured timeout policy, if any.
    #[must_use]
    pub fn timeout(&self) -> Option<&TimeoutPolicy> {
        self.timeout.as_ref()
    }
}

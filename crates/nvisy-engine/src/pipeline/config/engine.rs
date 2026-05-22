//! Engine-level execution policies, networking, and resource limits.

use nvisy_http::HttpConfig;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::workflow::{ConcurrencyPolicy, RetryPolicy, TimeoutPolicy};

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

    /// Maximum number of nodes allowed in an execution graph.
    ///
    /// Rejects overly complex graphs at compilation time. `None` means
    /// no limit (not yet enforced — reserved for future use).
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
/// by per-request or per-graph configuration.
///
/// # Field groups
///
/// **Execution policies** — applied to graph nodes that lack their own:
/// - [`retry`] — automatic retry on transient failures.
/// - [`timeout`] — per-node wall-clock deadline.
/// - [`concurrency`] — limits parallel node execution.
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
/// [`retry`]: Self::retry
/// [`timeout`]: Self::timeout
/// [`concurrency`]: Self::concurrency
/// [`http`]: Self::http
/// [`limits`]: Self::limits
/// [`cache`]: Self::cache
#[derive(Debug, Clone, Default, Validate, Serialize, Deserialize)]
pub struct EngineSection {
    /// Default retry policy applied to nodes without an explicit one.
    ///
    /// Controls max retries, delay, and backoff strategy for transient
    /// failures. Overridden by [`GraphNode::retry`] when set on an
    /// individual node.
    ///
    /// [`GraphNode::retry`]: crate::workflow::GraphNode::retry
    #[validate(nested)]
    pub retry: Option<RetryPolicy>,

    /// Default timeout policy applied to nodes without an explicit one.
    ///
    /// Sets a per-node wall-clock deadline and behavior on expiry
    /// (fail or skip). Overridden by [`GraphNode::timeout`] when set
    /// on an individual node.
    ///
    /// [`GraphNode::timeout`]: crate::workflow::GraphNode::timeout
    #[validate(nested)]
    pub timeout: Option<TimeoutPolicy>,

    /// Default concurrency limit for graph execution.
    ///
    /// Caps the number of graph nodes that may execute in parallel via
    /// a [`tokio::sync::Semaphore`]. Overridden by
    /// [`Graph::concurrency`] when set on the graph itself.
    ///
    /// [`Graph::concurrency`]: crate::workflow::Graph::concurrency
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
    /// Returns the configured retry policy, if any.
    #[must_use]
    pub fn retry(&self) -> Option<&RetryPolicy> {
        self.retry.as_ref()
    }

    /// Returns the configured timeout policy, if any.
    #[must_use]
    pub fn timeout(&self) -> Option<&TimeoutPolicy> {
        self.timeout.as_ref()
    }
}

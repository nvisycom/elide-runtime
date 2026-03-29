//! Engine-level execution policies, networking, and resource limits.

use nvisy_ontology::workflow::{ConcurrencyPolicy, RetryPolicy, TimeoutPolicy};
use nvisy_provider::http::HttpConfig;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Hard limits on pipeline resource consumption.
///
/// These values cap the overall duration and internal buffer sizes
/// for a single pipeline run. They are read once during
/// [`Engine::run`](super::super::Engine::run) and cannot be changed mid-run.
#[derive(Debug, Clone, Copy, Validate, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Hard ceiling on total pipeline run duration, in milliseconds.
    ///
    /// If a run exceeds this limit, the cancellation token is triggered
    /// and the run is marked as timed out. Individual node timeouts
    /// (via [`TimeoutPolicy`]) are independent of this limit.
    /// `None` means no run-level timeout.
    #[serde(default)]
    pub run_timeout_ms: Option<u64>,

    /// Buffer size for bounded MPSC channels between pipeline nodes.
    ///
    /// Controls backpressure: a larger buffer allows faster producers
    /// to run ahead of slower consumers. Defaults to 256.
    #[validate(range(min = 1, message = "channel_buffer must be at least 1"))]
    #[serde(default = "ResourceLimits::default_channel_buffer")]
    pub channel_buffer: usize,
}

impl ResourceLimits {
    pub(crate) const fn default_channel_buffer() -> usize {
        256
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            run_timeout_ms: None,
            channel_buffer: Self::default_channel_buffer(),
        }
    }
}

/// Engine-level execution policies, networking, and resource limits.
///
/// Controls default behavior for all pipeline runs unless overridden
/// by per-request or per-graph configuration.
///
/// # Field groups
///
/// **Execution policies** — applied to graph nodes that lack their own:
/// - [`retry`](Self::retry) — automatic retry on transient failures.
/// - [`timeout`](Self::timeout) — per-node wall-clock deadline.
/// - [`concurrency`](Self::concurrency) — limits parallel node execution.
///
/// **Networking:**
/// - [`http`](Self::http) — shared HTTP client settings (timeouts, retries,
///   connection pooling) for all downstream API calls (OCR, LLM, STT, etc.).
///
/// **Resource limits:**
/// - [`limits`](Self::limits) — run timeout and channel buffer size.
#[derive(Debug, Clone, Default, Validate, Serialize, Deserialize)]
pub struct EngineSection {
    /// Default retry policy applied to nodes without an explicit one.
    ///
    /// Controls max retries, delay, and backoff strategy for transient
    /// failures. Overridden by [`GraphNode::retry`](nvisy_ontology::workflow::GraphNode::retry)
    /// when set on an individual node. See [`RetryPolicy`] for details.
    #[validate(nested)]
    pub retry: Option<RetryPolicy>,

    /// Default timeout policy applied to nodes without an explicit one.
    ///
    /// Sets a per-node wall-clock deadline and behavior on expiry
    /// (fail or skip). Overridden by [`GraphNode::timeout`](nvisy_ontology::workflow::GraphNode::timeout)
    /// when set on an individual node. See [`TimeoutPolicy`] for details.
    #[validate(nested)]
    pub timeout: Option<TimeoutPolicy>,

    /// Default concurrency limit for graph execution.
    ///
    /// Caps the number of graph nodes that may execute in parallel via
    /// a [`tokio::sync::Semaphore`]. Overridden by
    /// [`Graph::concurrency`](nvisy_ontology::workflow::Graph::concurrency)
    /// when set on the graph itself.
    #[serde(default)]
    pub concurrency: Option<ConcurrencyPolicy>,

    /// Shared HTTP client configuration for all downstream API calls.
    ///
    /// Applies to OCR providers, LLM agents, STT/TTS services, and any
    /// other external HTTP dependencies. Controls timeouts, retries,
    /// and connection pooling.
    pub http: Option<HttpConfig>,

    /// Run-level resource limits (timeout, channel buffer size).
    ///
    /// Nested under `[engine.limits]` in TOML.
    #[validate(nested)]
    #[serde(default)]
    pub limits: ResourceLimits,
}

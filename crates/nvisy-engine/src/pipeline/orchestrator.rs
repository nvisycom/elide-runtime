//! DAG orchestrator: concurrent task scheduling for compiled execution plans.
//!
//! [`run_graph`] takes a compiled [`ExecutionPlan`] and spawns one tokio
//! task per node into a [`JoinSet`](tokio::task::JoinSet). Each task:
//!
//! 1. Waits for all upstream dependencies to complete via `watch::Receiver`
//!    channels (each initialized to `false`, flipped to `true` on completion).
//! 2. Acquires a permit from the optional concurrency [`Semaphore`](tokio::sync::Semaphore)
//!    (configured via [`ConcurrencyPolicy`](crate::graph::ConcurrencyPolicy)).
//! 3. Delegates to [`NodeExecutor::execute`] for the actual operation dispatch.
//! 4. Reports node status updates to the shared [`RunState`] for live
//!    progress visibility.
//!
//! [`CompletionGuard`] ensures the watch-channel signal is sent even if
//! the task panics, preventing downstream tasks from deadlocking.
//!
//! Data flows between nodes through bounded MPSC channels allocated from
//! the [`ResolvedEdge`](super::plan::ResolvedEdge) configuration.

use std::collections::HashMap;
use std::sync::Arc;

use nvisy_core::Error;
use nvisy_http::HttpClient;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::config::RuntimeConfig;
use super::executor::{NodeExecutor, NodeOutput, RunOutput};
use super::plan::ExecutionPlan;
use super::runs::NodeStatus;
use super::runs::state::RunState;
use crate::operation::DocumentEnvelope;
use crate::operation::context::SharedContext;

const TARGET: &str = "nvisy_engine::pipeline::orchestrator";

/// RAII guard that sends a `true` on its `watch::Sender` when dropped.
///
/// This ensures downstream tasks are unblocked even if the owning task
/// panics, preventing deadlocks in the DAG.
struct CompletionGuard {
    tx: Option<watch::Sender<bool>>,
}

impl CompletionGuard {
    fn new(tx: Option<watch::Sender<bool>>) -> Self {
        Self { tx }
    }
}

impl Drop for CompletionGuard {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(true);
        }
    }
}

/// Per-run execution context passed from [`Engine::run`](super::Engine::run)
/// to the orchestrator.
pub(super) struct RunContext {
    /// Token to signal cancellation to all node tasks.
    pub cancel: CancellationToken,
    /// Shared operation context (run ID, actor, registry, policies, key provider).
    pub shared: SharedContext,
    /// Effective configuration after merging per-request overrides.
    pub config: RuntimeConfig,
    /// Shared HTTP client for downstream API calls.
    pub http_client: HttpClient,
    /// Optional limit on how many nodes may execute concurrently.
    pub concurrency: Option<crate::graph::ConcurrencyPolicy>,
}

/// Execute a compiled [`ExecutionPlan`] by spawning concurrent tasks for
/// each node.
///
/// Each task updates its [`NodeSnapshot`] in the shared [`RunState`]
/// so that `GET /runs/{id}` reflects live progress.
///
/// [`NodeSnapshot`]: super::runs::NodeSnapshot
pub(super) async fn run_graph(
    plan: &ExecutionPlan,
    run_id: Uuid,
    runs: RunState,
    ctx: RunContext,
) -> Result<RunOutput, Error> {
    let RunContext {
        cancel,
        shared,
        config,
        http_client,
        concurrency,
    } = ctx;

    let semaphore = concurrency.map(|c| Arc::new(tokio::sync::Semaphore::new(c.max_nodes)));

    let mut senders: HashMap<Uuid, Vec<mpsc::Sender<Arc<DocumentEnvelope>>>> = HashMap::new();
    let mut receivers: HashMap<Uuid, Vec<mpsc::Receiver<Arc<DocumentEnvelope>>>> = HashMap::new();

    for edge in plan.edges() {
        let (tx, rx) = mpsc::channel(edge.config.channel_buffer);
        senders.entry(edge.source).or_default().push(tx);
        receivers.entry(edge.target).or_default().push(rx);
    }

    let mut signal_senders: HashMap<Uuid, watch::Sender<bool>> = HashMap::new();
    let mut signal_receivers: HashMap<Uuid, watch::Receiver<bool>> = HashMap::new();

    for resolved in plan.nodes() {
        let (tx, rx) = watch::channel(false);
        signal_senders.insert(resolved.node.id, tx);
        signal_receivers.insert(resolved.node.id, rx);
    }

    let mut join_set: JoinSet<NodeOutput> = JoinSet::new();

    for resolved in plan.nodes() {
        let resolved = resolved.clone();
        let node_id = resolved.node.id;
        let runs = runs.clone();

        let executor = NodeExecutor::new(
            shared.clone(),
            cancel.clone(),
            config.clone(),
            http_client.clone(),
        );

        let upstream_watches: Vec<watch::Receiver<bool>> = resolved
            .upstream_ids
            .iter()
            .filter_map(|id| signal_receivers.get(id).cloned())
            .collect();

        let completion_tx = signal_senders.remove(&node_id);
        let node_senders = senders.remove(&node_id).unwrap_or_default();
        let node_receivers = receivers.remove(&node_id).unwrap_or_default();
        let sem = semaphore.clone();

        join_set.spawn(async move {
            let _guard = CompletionGuard::new(completion_tx);

            for mut rx in upstream_watches {
                let _ = rx.wait_for(|&done| done).await;
            }
            tracing::trace!(target: TARGET, %node_id, "all upstream dependencies satisfied");

            let _permit = if let Some(ref sem) = sem {
                Some(sem.acquire().await.expect("semaphore closed"))
            } else {
                None
            };

            tracing::debug!(target: TARGET, %node_id, "node starting");
            runs.update_node(run_id, node_id, NodeStatus::Running, 0, None)
                .await;

            let result = executor
                .execute(&resolved, node_senders, node_receivers)
                .await;

            let output = match result {
                Ok(output) => output,
                Err(e) => NodeOutput {
                    items_processed: 0,
                    error: Some(e.to_string()),
                    envelopes: Vec::new(),
                },
            };

            let status = if output.error.is_none() {
                NodeStatus::Succeeded
            } else {
                NodeStatus::Failed
            };
            tracing::debug!(target: TARGET, %node_id, ?status, items = output.items_processed, "node completed");
            runs.update_node(
                run_id,
                node_id,
                status,
                output.items_processed,
                output.error.clone(),
            )
            .await;

            output
        });
    }

    let mut node_results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(nr) => node_results.push(nr),
            Err(e) => {
                let msg = if e.is_panic() {
                    let panic_payload = e.into_panic();
                    let panic_msg = panic_payload
                        .downcast_ref::<&str>()
                        .copied()
                        .or_else(|| panic_payload.downcast_ref::<String>().map(|s| s.as_str()))
                        .unwrap_or("unknown panic");
                    tracing::error!(
                        target: TARGET,
                        panic = panic_msg,
                        "node task panicked"
                    );
                    format!("Task panicked: {panic_msg}")
                } else {
                    tracing::error!(
                        target: TARGET,
                        error = %e,
                        "node task failed"
                    );
                    format!("Task failed: {e}")
                };
                node_results.push(NodeOutput {
                    items_processed: 0,
                    error: Some(msg),
                    envelopes: Vec::new(),
                });
            }
        }
    }

    Ok(RunOutput { node_results })
}

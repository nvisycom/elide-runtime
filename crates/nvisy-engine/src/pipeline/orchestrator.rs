//! DAG orchestrator: concurrent task scheduling for compiled execution plans.
//!
//! [`Orchestrator`] takes a compiled [`ExecutionPlan`] and spawns one
//! tokio task per node. Each task:
//!
//! 1. Waits for upstream dependencies via `watch::Receiver` channels.
//! 2. Acquires a permit from the optional concurrency semaphore.
//! 3. Delegates to [`NodeExecutor::execute`] for operation dispatch.
//! 4. Reports node status updates to the shared [`RunState`].
//!
//! [`CompletionGuard`] ensures the watch-channel signal fires even if
//! the task panics, preventing downstream deadlocks.

use std::collections::HashMap;
use std::sync::Arc;

use nvisy_core::Error;
use nvisy_ontology::workflow::ConcurrencyPolicy;
use nvisy_provider::http::HttpClient;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::config::RuntimeConfig;
use super::executor::{NodeExecutor, NodeOutput, RunOutput};
use super::plan::ExecutionPlan;
use super::runs::NodeStatus;
use super::runs::state::RunState;
use crate::graph::ConcurrencyExt;
use crate::operation::DocumentEnvelope;
use crate::operation::envelope::SharedData;

const TARGET: &str = "nvisy_engine::pipeline::orchestrator";

type ChannelMap<T> = HashMap<Uuid, Vec<T>>;

/// RAII guard that signals completion on drop, preventing deadlocks.
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

/// Per-run execution context shared across all node tasks.
pub(super) struct RunContext {
    /// Token to signal cancellation to all node tasks.
    pub cancel: CancellationToken,
    /// Shared run-wide state: run ID, actor, registry, policies, key provider.
    pub shared: Arc<SharedData>,
    /// Effective configuration after merging per-request overrides.
    pub config: Arc<RuntimeConfig>,
    /// Shared HTTP client for downstream API calls.
    pub http_client: HttpClient,
    /// Optional limit on how many nodes may execute concurrently.
    pub concurrency: Option<ConcurrencyPolicy>,
    /// When `true`, skip validation and export phases.
    pub dry_run: bool,
}

/// DAG orchestrator: spawns concurrent node tasks and collects results.
pub(super) struct Orchestrator {
    ctx: Arc<RunContext>,
    run_id: Uuid,
    runs: RunState,
    semaphore: Option<Arc<tokio::sync::Semaphore>>,
}

impl Orchestrator {
    /// Create an orchestrator for the given run.
    pub fn new(run_id: Uuid, runs: RunState, ctx: RunContext) -> Self {
        let semaphore = ctx.concurrency.map(|c| c.to_semaphore());
        Self {
            ctx: Arc::new(ctx),
            run_id,
            runs,
            semaphore,
        }
    }

    /// Execute the compiled plan, returning results from all nodes.
    pub async fn run(self, plan: &ExecutionPlan) -> Result<RunOutput, Error> {
        let (senders, receivers) = self.build_channels(plan);
        let (signal_senders, signal_receivers) = self.build_signals(plan);
        let join_set = self.spawn_tasks(plan, senders, receivers, signal_senders, signal_receivers);
        self.collect_results(join_set).await
    }

    /// Build MPSC data channels from plan edges.
    fn build_channels(
        &self,
        plan: &ExecutionPlan,
    ) -> (
        ChannelMap<mpsc::Sender<DocumentEnvelope>>,
        ChannelMap<mpsc::Receiver<DocumentEnvelope>>,
    ) {
        let mut senders: ChannelMap<mpsc::Sender<DocumentEnvelope>> = HashMap::new();
        let mut receivers: ChannelMap<mpsc::Receiver<DocumentEnvelope>> = HashMap::new();
        for edge in plan.edges() {
            let (tx, rx) = mpsc::channel(edge.config.channel_buffer);
            senders.entry(edge.source).or_default().push(tx);
            receivers.entry(edge.target).or_default().push(rx);
        }
        (senders, receivers)
    }

    /// Build watch-channel dependency signals for each node.
    fn build_signals(
        &self,
        plan: &ExecutionPlan,
    ) -> (
        HashMap<Uuid, watch::Sender<bool>>,
        HashMap<Uuid, watch::Receiver<bool>>,
    ) {
        let mut senders = HashMap::new();
        let mut receivers = HashMap::new();
        for resolved in plan.nodes() {
            let (tx, rx) = watch::channel(false);
            senders.insert(resolved.node.id, tx);
            receivers.insert(resolved.node.id, rx);
        }
        (senders, receivers)
    }

    /// Spawn one task per node into a JoinSet.
    fn spawn_tasks(
        &self,
        plan: &ExecutionPlan,
        mut senders: ChannelMap<mpsc::Sender<DocumentEnvelope>>,
        mut receivers: ChannelMap<mpsc::Receiver<DocumentEnvelope>>,
        mut signal_senders: HashMap<Uuid, watch::Sender<bool>>,
        signal_receivers: HashMap<Uuid, watch::Receiver<bool>>,
    ) -> JoinSet<NodeOutput> {
        let mut join_set = JoinSet::new();

        for resolved in plan.nodes() {
            let resolved = resolved.clone();
            let node_id = resolved.node.id;
            let runs = self.runs.clone();
            let run_id = self.run_id;
            let cancel = self.ctx.cancel.clone();
            let executor = NodeExecutor::new(Arc::clone(&self.ctx));
            let sem = self.semaphore.clone();

            let upstream_watches: Vec<_> = resolved
                .upstream_ids
                .iter()
                .filter_map(|id| signal_receivers.get(id).cloned())
                .collect();
            let completion_tx = signal_senders.remove(&node_id);
            let node_senders = senders.remove(&node_id).unwrap_or_default();
            let node_receivers = receivers.remove(&node_id).unwrap_or_default();

            join_set.spawn(async move {
                let _guard = CompletionGuard::new(completion_tx);

                for mut rx in upstream_watches {
                    let _ = rx.wait_for(|&done| done).await;
                }
                tracing::trace!(target: TARGET, %node_id, "upstream dependencies satisfied");

                let _permit = match sem {
                    Some(ref s) => Some(s.acquire().await.expect("semaphore closed")),
                    None => None,
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
                    cancel.cancel();
                    NodeStatus::Failed
                };

                tracing::debug!(
                    target: TARGET,
                    %node_id, ?status,
                    items = output.items_processed,
                    "node completed",
                );
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

        join_set
    }

    /// Collect results from all spawned tasks.
    async fn collect_results(&self, mut join_set: JoinSet<NodeOutput>) -> Result<RunOutput, Error> {
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
                        tracing::error!(target: TARGET, panic = panic_msg, "node task panicked");
                        format!("Task panicked: {panic_msg}")
                    } else {
                        tracing::error!(target: TARGET, error = %e, "node task failed");
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
}

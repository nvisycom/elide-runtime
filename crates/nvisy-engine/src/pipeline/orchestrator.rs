//! DAG orchestrator: spawns concurrent node tasks and collects results.
//!
//! [`run_graph`] takes a compiled [`ExecutionPlan`] and spawns one tokio
//! task per node. Tasks wait for upstream completion via watch channels
//! before executing, and report progress to the shared [`RunState`].

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
use super::runs::state::RunState;
use super::runs::NodeStatus;
use crate::operation::DocumentEnvelope;
use crate::operation::context::SharedContext;

/// Per-run execution context.
pub(super) struct RunContext {
    pub cancel: CancellationToken,
    pub shared: SharedContext,
    pub config: RuntimeConfig,
    pub http_client: HttpClient,
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
    } = ctx;

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

        join_set.spawn(async move {
            for mut rx in upstream_watches {
                let _ = rx.wait_for(|&done| done).await;
            }

            runs.update_node(run_id, node_id, NodeStatus::Running, 0, None)
                .await;

            let result = executor
                .execute(&resolved, node_senders, node_receivers)
                .await;

            if let Some(tx) = completion_tx {
                let _ = tx.send(true);
            }

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
            Err(e) => node_results.push(NodeOutput {
                items_processed: 0,
                error: Some(format!("Task panicked: {e}")),
                envelopes: Vec::new(),
            }),
        }
    }

    Ok(RunOutput { node_results })
}

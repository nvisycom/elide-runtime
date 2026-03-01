//! Graph runner that executes a compiled [`ExecutionPlan`].
//!
//! Each node is spawned as a concurrent Tokio task. Data flows between nodes
//! via bounded MPSC channels, and upstream completion is signalled via watch
//! channels so downstream tasks wait before starting.
//!
//! [`execute_node`] dispatches to variant-specific handlers:
//!
//! | Variant  | Behaviour                                              |
//! |----------|--------------------------------------------------------|
//! | `Source` | Reads data from an external connection and sends it downstream. |
//! | `Action` | Receives upstream data, applies a transformation, and forwards results. |
//! | `Target` | Receives upstream data and writes it to an external connection. |

use std::collections::HashMap;
use std::sync::Arc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use uuid::Uuid;
use nvisy_core::io::ContentData;
use nvisy_core::{Error, ErrorKind};
use crate::compiler::plan::ExecutionPlan;
use crate::compiler::graph::{ActionKind, GraphNode, GraphNodeKind, TimeoutBehavior};
use crate::compiler::RetryPolicy;
use super::connections::{Connection, Connections};
use super::policies::{with_retry, with_timeout};

/// Default buffer size for bounded inter-node MPSC channels.
const CHANNEL_BUFFER_SIZE: usize = 256;

/// Outcome of executing a single node in the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodeOutput {
    /// ID of the node that produced this result.
    pub node_id: Uuid,
    /// Number of data items processed by this node.
    pub items_processed: u64,
    /// Error message if the node failed, or `None` on success.
    pub error: Option<String>,
}

/// Aggregate outcome of executing an entire pipeline graph.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RunOutput {
    /// Unique identifier for this execution run.
    pub run_id: Uuid,
    /// Per-node results in completion order.
    pub node_results: Vec<NodeOutput>,
    /// `true` if all nodes completed without error.
    pub success: bool,
}

/// Executes a compiled [`ExecutionPlan`] by spawning concurrent tasks for each node.
///
/// Returns a [`RunOutput`] containing per-node outcomes and an overall success flag.
pub(crate) async fn run_graph(
    plan: &ExecutionPlan,
    connections: &Connections,
) -> Result<RunOutput, Error> {
    let run_id = Uuid::new_v4();
    let connections = Arc::new(connections.clone());

    // Create channels for each edge
    let mut senders: HashMap<Uuid, Vec<mpsc::Sender<ContentData>>> = HashMap::new();
    let mut receivers: HashMap<Uuid, Vec<mpsc::Receiver<ContentData>>> = HashMap::new();

    for node in &plan.nodes {
        let node_id = node.node.id;
        for downstream_id in &node.downstream_ids {
            let (tx, rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);
            senders.entry(node_id).or_default().push(tx);
            receivers.entry(*downstream_id).or_default().push(rx);
        }
    }

    // Create completion signals per node
    let mut signal_senders: HashMap<Uuid, watch::Sender<bool>> = HashMap::new();
    let mut signal_receivers: HashMap<Uuid, watch::Receiver<bool>> = HashMap::new();

    for node in &plan.nodes {
        let (tx, rx) = watch::channel(false);
        signal_senders.insert(node.node.id, tx);
        signal_receivers.insert(node.node.id, rx);
    }

    // Spawn tasks
    let mut join_set: JoinSet<NodeOutput> = JoinSet::new();

    for resolved in &plan.nodes {
        let node = resolved.node.clone();
        let node_id = node.id;
        let upstream_ids = resolved.upstream_ids.clone();

        // Collect upstream watch receivers
        let upstream_watches: Vec<watch::Receiver<bool>> = upstream_ids
            .iter()
            .filter_map(|id| signal_receivers.get(id).cloned())
            .collect();

        let completion_tx = signal_senders.remove(&node_id);
        let node_senders = senders.remove(&node_id).unwrap_or_default();
        let node_receivers = receivers.remove(&node_id).unwrap_or_default();
        let conns = Arc::clone(&connections);

        join_set.spawn(async move {
            // Wait for upstream nodes to complete
            for mut rx in upstream_watches {
                let _ = rx.wait_for(|&done| done).await;
            }

            let result = execute_node(&node, node_senders, node_receivers, &conns).await;

            // Signal completion
            if let Some(tx) = completion_tx {
                let _ = tx.send(true);
            }

            match result {
                Ok(count) => NodeOutput {
                    node_id,
                    items_processed: count,
                    error: None,
                },
                Err(e) => NodeOutput {
                    node_id,
                    items_processed: 0,
                    error: Some(e.to_string()),
                },
            }
        });
    }

    // Collect results
    let mut node_results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(nr) => node_results.push(nr),
            Err(e) => node_results.push(NodeOutput {
                node_id: Uuid::nil(),
                items_processed: 0,
                error: Some(format!("Task panicked: {}", e)),
            }),
        }
    }

    let success = node_results.iter().all(|r| r.error.is_none());

    Ok(RunOutput {
        run_id,
        node_results,
        success,
    })
}

/// Execute a single node, dispatching to the correct handler based on the
/// [`GraphNodeKind`] variant.
///
/// A per-node timeout is applied when configured. The [`TimeoutBehavior`]
/// determines whether a timeout is treated as an error (`Fail`) or silently
/// yields zero items (`Skip`). Retry policies are applied within the
/// individual source/target handlers where the retryable I/O actually
/// occurs (channel consumption is not retryable).
async fn execute_node(
    node: &GraphNode,
    senders: Vec<mpsc::Sender<ContentData>>,
    mut receivers: Vec<mpsc::Receiver<ContentData>>,
    connections: &Connections,
) -> Result<u64, Error> {
    let run = async {
        match &node.kind {
            GraphNodeKind::Source(src) => {
                execute_source(
                    &src.provider, &src.stream,
                    node.retry(), &senders, connections,
                ).await
            }
            GraphNodeKind::Action(act) => {
                execute_action(&act.action, &senders, &mut receivers).await
            }
            GraphNodeKind::Target(tgt) => {
                execute_target(
                    &tgt.provider, &tgt.stream,
                    node.retry(), &mut receivers, connections,
                ).await
            }
        }
    };

    // Apply per-node timeout when configured.
    match node.timeout() {
        Some(policy) => {
            let result = with_timeout(policy.duration_ms, run).await;
            match (&result, &policy.on_timeout) {
                (Err(e), TimeoutBehavior::Skip) if e.kind == ErrorKind::Timeout => Ok(0),
                _ => result,
            }
        }
        None => run.await,
    }
}

/// Resolve a connection by provider name, returning an error if not found.
fn resolve_connection<'a>(
    provider: &str,
    connections: &'a Connections,
) -> Result<&'a Connection, Error> {
    connections.get(provider).ok_or_else(|| {
        Error::new(
            ErrorKind::Validation,
            format!("No connection configured for provider '{provider}'"),
        )
        .with_component("executor")
    })
}

/// Execute a `Source` node: read data from an external provider and send
/// items downstream.
///
/// Resolves the named connection and applies the retry policy to the
/// provider read operation. Actual provider integration (S3, database, etc.)
/// is not yet implemented — source nodes currently produce no data.
async fn execute_source(
    provider: &str,
    stream: &str,
    retry: Option<&RetryPolicy>,
    senders: &[mpsc::Sender<ContentData>],
    connections: &Connections,
) -> Result<u64, Error> {
    let _conn = resolve_connection(provider, connections)?;

    let read_from_provider = || async {
        tracing::debug!(provider, stream, "source node: reading from provider");

        // TODO: Dispatch to provider-specific readers (S3, database, etc.)
        // For now, source nodes produce no data. The Engine wrapper injects
        // initial content into the graph via the first channel directly.
        Ok::<u64, Error>(0)
    };

    let count = match retry {
        Some(policy) => with_retry(policy, read_from_provider).await?,
        None => read_from_provider().await?,
    };

    // Send items downstream once we have them.
    // (Currently a no-op since providers are not yet wired.)
    let _ = senders;

    Ok(count)
}

/// Execute an `Action` node: receive upstream data, apply a transformation,
/// and forward the result downstream.
///
/// Concrete action dispatch (detect, classify, redact) is orchestrated by
/// [`DefaultEngine::run`] which drives detection -> evaluation -> redaction
/// as sequential phases. The channel-level passthrough here handles any
/// action nodes that appear in the DAG but whose logic is managed externally.
async fn execute_action(
    action: &ActionKind,
    senders: &[mpsc::Sender<ContentData>],
    receivers: &mut [mpsc::Receiver<ContentData>],
) -> Result<u64, Error> {
    tracing::debug!(?action, "action node: processing");

    // Forward items from all upstream receivers to all downstream senders.
    let mut count = 0u64;
    for rx in receivers.iter_mut() {
        while let Some(item) = rx.recv().await {
            count += 1;
            for tx in senders {
                let _ = tx.send(item.clone()).await;
            }
        }
    }

    Ok(count)
}

/// Execute a `Target` node: consume upstream data and write to an external
/// provider.
///
/// Resolves the named connection and applies the retry policy to the
/// provider write operation. Actual provider integration is not yet
/// implemented — target nodes currently consume and count items.
async fn execute_target(
    provider: &str,
    stream: &str,
    retry: Option<&RetryPolicy>,
    receivers: &mut [mpsc::Receiver<ContentData>],
    connections: &Connections,
) -> Result<u64, Error> {
    let _conn = resolve_connection(provider, connections)?;

    tracing::debug!(provider, stream, "target node: writing to provider");

    // Consume all upstream items.
    let mut count = 0u64;
    for rx in receivers.iter_mut() {
        while let Some(_item) = rx.recv().await {
            count += 1;

            // TODO: Dispatch to provider-specific writers (S3, database, etc.)
            // with retry support. For now we just count items consumed.
        }
    }

    let _ = retry; // Will be used when provider writes are implemented.

    Ok(count)
}

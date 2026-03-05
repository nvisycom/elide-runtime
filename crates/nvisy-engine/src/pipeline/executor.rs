//! Node-level execution dispatchers.
//!
//! [`execute_node`] dispatches to variant-specific handlers:
//!
//! | Variant  | Behaviour                                              |
//! |----------|--------------------------------------------------------|
//! | `Source` | Reads data from an external provider and sends it downstream. |
//! | `Action` | Receives upstream data, applies a transformation, and forwards results. |
//! | `Target` | Receives upstream data and writes it to an external provider. |

use nvisy_core::io::ContentData;
use nvisy_core::{Error, ErrorKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::policy::{CompiledRetryPolicy, CompiledTimeoutPolicy};
use crate::compiler::{ActionKind, GraphNode, GraphNodeKind, RetryPolicy, TimeoutBehavior};

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

/// Execute a single node, dispatching to the correct handler based on the
/// [`GraphNodeKind`] variant.
///
/// A per-node timeout is applied when configured. The [`TimeoutBehavior`]
/// determines whether a timeout is treated as an error (`Fail`) or silently
/// yields zero items (`Skip`). Retry policies are applied within the
/// individual source/target handlers where the retryable I/O actually
/// occurs (channel consumption is not retryable).
pub(crate) async fn execute_node(
    node: &GraphNode,
    senders: Vec<mpsc::Sender<ContentData>>,
    mut receivers: Vec<mpsc::Receiver<ContentData>>,
) -> Result<u64, Error> {
    let run = async {
        match &node.kind {
            GraphNodeKind::Source(src) => {
                execute_source(&src.provider, &src.stream, node.retry(), &senders).await
            }
            GraphNodeKind::Action(act) => {
                execute_action(&act.action, &senders, &mut receivers).await
            }
            GraphNodeKind::Target(tgt) => {
                execute_target(&tgt.provider, &tgt.stream, node.retry(), &mut receivers).await
            }
        }
    };

    // Apply per-node timeout when configured.
    match node.timeout() {
        Some(policy) => {
            let compiled = CompiledTimeoutPolicy::from(policy);
            let result = compiled.with_timeout(run).await;
            match (&result, &compiled.on_timeout) {
                (Err(e), TimeoutBehavior::Skip) if e.kind == ErrorKind::Timeout => Ok(0),
                _ => result,
            }
        }
        None => run.await,
    }
}

/// Execute a `Source` node: read data from an external provider and send
/// items downstream.
///
/// Actual provider integration (S3, database, etc.) is not yet implemented —
/// source nodes currently produce no data.
async fn execute_source(
    provider: &str,
    stream: &str,
    retry: Option<&RetryPolicy>,
    senders: &[mpsc::Sender<ContentData>],
) -> Result<u64, Error> {
    let read_from_provider = || async {
        tracing::debug!(provider, stream, "source node: reading from provider");

        // TODO: Dispatch to provider-specific readers (S3, database, etc.)
        // For now, source nodes produce no data. The Engine wrapper injects
        // initial content into the graph via the first channel directly.
        Ok::<u64, Error>(0)
    };

    let count = match retry {
        Some(policy) => {
            CompiledRetryPolicy::from(policy)
                .with_retry(read_from_provider)
                .await?
        }
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
    match action {
        ActionKind::Ocr => tracing::trace!("action node: ocr (passthrough)"),
        ActionKind::Transcribe => tracing::trace!("action node: transcribe (passthrough)"),
        ActionKind::Detect => tracing::trace!("action node: detect (passthrough)"),
        ActionKind::Evaluate => tracing::trace!("action node: evaluate (passthrough)"),
        ActionKind::Redact => tracing::trace!("action node: redact (passthrough)"),
        ActionKind::Translate => tracing::trace!("action node: translate (passthrough)"),
        ActionKind::Classify => tracing::trace!("action node: classify (passthrough)"),
        ActionKind::Summarize => tracing::trace!("action node: summarize (passthrough)"),
        ActionKind::Audit => tracing::trace!("action node: audit (passthrough)"),
        ActionKind::Publish => tracing::trace!("action node: publish (passthrough)"),
    }

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
/// Actual provider integration is not yet implemented — target nodes
/// currently consume and count items.
async fn execute_target(
    provider: &str,
    stream: &str,
    retry: Option<&RetryPolicy>,
    receivers: &mut [mpsc::Receiver<ContentData>],
) -> Result<u64, Error> {
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

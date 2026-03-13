//! Node-level execution dispatchers.
//!
//! [`execute_node`] dispatches each graph node to the appropriate handler
//! based on its [`GraphNodeKind`]. A per-node timeout is applied when
//! configured, with [`TimeoutBehavior`] controlling whether a timeout
//! is treated as an error or silently yields zero items.

use nvisy_core::content::ContentData;
use nvisy_core::{Error, ErrorKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::policy::CompiledTimeoutPolicy;
use crate::compiler::{GraphNode, GraphNodeKind, TimeoutBehavior};

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

/// Executes a single graph node by dispatching on its [`GraphNodeKind`].
///
/// A per-node timeout is applied when configured. The [`TimeoutBehavior`]
/// determines whether a timeout is treated as an error (`Fail`) or silently
/// yields zero items (`Skip`).
pub(crate) async fn execute_node(
    node: &GraphNode,
    senders: Vec<mpsc::Sender<ContentData>>,
    mut receivers: Vec<mpsc::Receiver<ContentData>>,
) -> Result<u64, Error> {
    let run = async { execute_action(&node.kind, &senders, &mut receivers).await };

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

/// Dispatches an action node: receives upstream data, logs the action kind,
/// and forwards items downstream.
///
/// Concrete action implementations will replace these passthrough stubs
/// as the orchestrator is built out.
async fn execute_action(
    action: &GraphNodeKind,
    senders: &[mpsc::Sender<ContentData>],
    receivers: &mut [mpsc::Receiver<ContentData>],
) -> Result<u64, Error> {
    match action {
        GraphNodeKind::LoadContext(_) => tracing::trace!("action node: load_context (passthrough)"),
        GraphNodeKind::SaveContext(_) => tracing::trace!("action node: save_context (passthrough)"),
        GraphNodeKind::GenerateContext(_) => {
            tracing::trace!("action node: generate_context (passthrough)")
        }
        GraphNodeKind::VisualExtraction(_) => {
            tracing::trace!("action node: visual_extraction (passthrough)")
        }
        GraphNodeKind::AudialExtraction(_) => {
            tracing::trace!("action node: audial_extraction (passthrough)")
        }
        GraphNodeKind::NamedEntityRecognition(_) => {
            tracing::trace!("action node: ner (passthrough)")
        }
        GraphNodeKind::PatternRecognition(_) => {
            tracing::trace!("action node: pattern_recognition (passthrough)")
        }
        GraphNodeKind::Fusion(_) => tracing::trace!("action node: fusion (passthrough)"),
        GraphNodeKind::Redaction(_) => tracing::trace!("action node: redaction (passthrough)"),
        GraphNodeKind::Import(_) => tracing::trace!("action node: import (passthrough)"),
        GraphNodeKind::Export(_) => tracing::trace!("action node: export (passthrough)"),
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

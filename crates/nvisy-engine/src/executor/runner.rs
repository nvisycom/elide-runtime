use std::collections::HashMap;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use uuid::Uuid;
use nvisy_core::data::DataValue;
use nvisy_core::errors::NvisyError;
use nvisy_core::registry::Registry;
use crate::compiler::plan::ExecutionPlan;
use crate::connections::Connections;
use crate::executor::context::CHANNEL_BUFFER_SIZE;
use crate::schema::GraphNode;

/// Result of a single node execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeResult {
    pub node_id: String,
    pub items_processed: u64,
    pub error: Option<String>,
}

/// Result of an entire graph execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunResult {
    pub run_id: Uuid,
    pub node_results: Vec<NodeResult>,
    pub success: bool,
}

/// Execute a compiled graph plan.
pub async fn run_graph(
    plan: &ExecutionPlan,
    _connections: &Connections,
    _registry: &Registry,
) -> Result<RunResult, NvisyError> {
    let run_id = Uuid::new_v4();

    // Create channels for each edge
    // Key: "from_id -> to_id", value: (sender, receiver)
    let mut senders: HashMap<String, Vec<mpsc::Sender<DataValue>>> = HashMap::new();
    let mut receivers: HashMap<String, Vec<mpsc::Receiver<DataValue>>> = HashMap::new();

    for node in &plan.nodes {
        let node_id = node.node.id();
        for downstream_id in &node.downstream_ids {
            let (tx, rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);
            senders.entry(node_id.to_string()).or_default().push(tx);
            receivers.entry(downstream_id.clone()).or_default().push(rx);
        }
    }

    // Create completion signals per node
    let mut signal_senders: HashMap<String, watch::Sender<bool>> = HashMap::new();
    let mut signal_receivers: HashMap<String, watch::Receiver<bool>> = HashMap::new();

    for node in &plan.nodes {
        let (tx, rx) = watch::channel(false);
        signal_senders.insert(node.node.id().to_string(), tx);
        signal_receivers.insert(node.node.id().to_string(), rx);
    }

    // Spawn tasks
    let mut join_set: JoinSet<NodeResult> = JoinSet::new();

    for resolved in &plan.nodes {
        let node = resolved.node.clone();
        let node_id = node.id().to_string();
        let upstream_ids = resolved.upstream_ids.clone();

        // Collect upstream watch receivers
        let upstream_watches: Vec<watch::Receiver<bool>> = upstream_ids
            .iter()
            .filter_map(|id| signal_receivers.get(id).cloned())
            .collect();

        let completion_tx = signal_senders.remove(&node_id);
        let node_senders = senders.remove(&node_id).unwrap_or_default();
        let node_receivers = receivers.remove(&node_id).unwrap_or_default();

        join_set.spawn(async move {
            // Wait for upstream nodes to complete
            for mut rx in upstream_watches {
                let _ = rx.wait_for(|&done| done).await;
            }

            let result = execute_node(&node, node_senders, node_receivers).await;

            // Signal completion
            if let Some(tx) = completion_tx {
                let _ = tx.send(true);
            }

            match result {
                Ok(count) => NodeResult {
                    node_id,
                    items_processed: count,
                    error: None,
                },
                Err(e) => NodeResult {
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
            Err(e) => node_results.push(NodeResult {
                node_id: "unknown".to_string(),
                items_processed: 0,
                error: Some(format!("Task panicked: {}", e)),
            }),
        }
    }

    let success = node_results.iter().all(|r| r.error.is_none());

    Ok(RunResult {
        run_id,
        node_results,
        success,
    })
}

/// Execute a single node with its channels (simplified -- does not use registry directly).
async fn execute_node(
    _node: &GraphNode,
    senders: Vec<mpsc::Sender<DataValue>>,
    mut receivers: Vec<mpsc::Receiver<DataValue>>,
) -> Result<u64, NvisyError> {
    // For now, forward items from receivers to senders (passthrough behavior).
    // The actual registry-based dispatch happens via the Engine wrapper.
    let mut count = 0u64;

    for rx in &mut receivers {
        while let Some(item) = rx.recv().await {
            count += 1;
            for tx in &senders {
                let _ = tx.send(item.clone()).await;
            }
        }
    }

    // Drop senders to signal downstream completion
    drop(senders);

    Ok(count)
}

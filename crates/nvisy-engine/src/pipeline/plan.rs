//! Compilation of a [`Graph`] into an [`ExecutionPlan`].
//!
//! [`compile()`] is the bridge between the user-facing [`Graph`] definition
//! and the runtime execution model. It performs the following steps:
//!
//! 1. **Policy defaults** — nodes without explicit retry/timeout policies
//!    inherit the engine-level defaults.
//! 2. **Validation** — the graph is checked for structural correctness
//!    (via [`Graph::validate`]).
//! 3. **petgraph construction** — nodes and edges are inserted into a
//!    [`DiGraph`](petgraph::graph::DiGraph) for cycle detection and
//!    topological sorting.
//! 4. **Topological sort** — produces the node execution order, ensuring
//!    every node runs after its dependencies.
//! 5. **Edge resolution** — each edge is paired with an [`EdgeConfig`]
//!    controlling the bounded MPSC channel buffer size.
//!
//! The resulting [`ExecutionPlan`] is consumed by the
//! [orchestrator](super::orchestrator) to spawn concurrent tasks.

use nvisy_core::{Error, Result};
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use uuid::Uuid;

use crate::graph::{Graph, GraphEdge, GraphNode, RetryPolicy, TimeoutPolicy};

/// Default buffer size for bounded MPSC channels between nodes.
const DEFAULT_CHANNEL_BUFFER: usize = 256;

/// Channel configuration for a resolved edge.
#[derive(Debug, Clone)]
pub struct EdgeConfig {
    /// Buffer size for the bounded MPSC channel on this edge.
    pub channel_buffer: usize,
}

impl Default for EdgeConfig {
    fn default() -> Self {
        Self {
            channel_buffer: DEFAULT_CHANNEL_BUFFER,
        }
    }
}

/// A directed edge with pre-computed channel configuration.
#[derive(Debug, Clone)]
pub struct ResolvedEdge {
    /// ID of the upstream node.
    pub source: Uuid,
    /// ID of the downstream node.
    pub target: Uuid,
    /// Channel configuration for this edge.
    pub config: EdgeConfig,
}

/// A graph node enriched with adjacency information and compiled policies.
///
/// Order is implicit in the position within [`ExecutionPlan::nodes`].
#[derive(Debug, Clone)]
pub struct ResolvedNode {
    /// The original graph node definition.
    pub node: GraphNode,
    /// IDs of nodes that feed data into this node.
    pub upstream_ids: Vec<Uuid>,
    /// Retry policy for this node, if configured.
    pub retry: Option<RetryPolicy>,
    /// Timeout policy for this node, if configured.
    pub timeout: Option<TimeoutPolicy>,
}

/// Compiles a [`Graph`] into an [`ExecutionPlan`].
///
/// Validates the graph, applies default policies to nodes that don't specify
/// their own, builds a petgraph representation, checks for cycles, and
/// produces a topologically-sorted plan.
pub(crate) fn compile(
    mut graph: Graph,
    default_retry: Option<&RetryPolicy>,
    default_timeout: Option<&TimeoutPolicy>,
    channel_buffer: Option<usize>,
) -> Result<ExecutionPlan> {
    for node in &mut graph.nodes {
        if node.retry.is_none() {
            node.retry = default_retry.cloned();
        }
        if node.timeout.is_none() {
            node.timeout = default_timeout.cloned();
        }
    }

    graph.validate()?;

    let pg = build_petgraph(&graph);

    let topo =
        toposort(&pg, None).map_err(|_| Error::validation("graph contains a cycle", "compiler"))?;

    Ok(ExecutionPlan::from_graph(
        &pg,
        &topo,
        channel_buffer.unwrap_or(DEFAULT_CHANNEL_BUFFER),
    ))
}

/// Build a petgraph `DiGraph` from a validated [`Graph`], mapping node
/// IDs to indices for edge wiring.
fn build_petgraph(graph: &Graph) -> DiGraph<GraphNode, GraphEdge> {
    let mut pg = DiGraph::with_capacity(graph.nodes.len(), graph.edges.len());
    let mut index_map = std::collections::HashMap::with_capacity(graph.nodes.len());

    for node in &graph.nodes {
        let idx = pg.add_node(node.clone());
        index_map.insert(node.id, idx);
    }

    for edge in &graph.edges {
        let from = *index_map
            .get(&edge.source)
            .expect("edge source must reference a node present in the graph");
        let to = *index_map
            .get(&edge.target)
            .expect("edge target must reference a node present in the graph");
        pg.add_edge(from, to, edge.clone());
    }

    pg
}

/// A compiled execution plan ready for the executor.
///
/// Contains all nodes in topological order and edges with channel
/// configuration. Constructed only via [`compile()`].
pub struct ExecutionPlan {
    nodes: Vec<ResolvedNode>,
    edges: Vec<ResolvedEdge>,
}

impl ExecutionPlan {
    /// Builds an execution plan from a petgraph and its topological ordering.
    fn from_graph(
        pg: &DiGraph<GraphNode, GraphEdge>,
        topo: &[NodeIndex],
        channel_buffer: usize,
    ) -> Self {
        let mut nodes = Vec::with_capacity(topo.len());

        for &idx in topo {
            let graph_node = &pg[idx];
            let upstream_ids: Vec<Uuid> = pg
                .neighbors_directed(idx, petgraph::Direction::Incoming)
                .map(|n| pg[n].id)
                .collect();

            let retry = graph_node.retry().cloned();
            let timeout = graph_node.timeout().cloned();

            nodes.push(ResolvedNode {
                node: graph_node.clone(),
                upstream_ids,
                retry,
                timeout,
            });
        }

        let edges: Vec<ResolvedEdge> = pg
            .edge_indices()
            .map(|ei| {
                let (src_idx, tgt_idx) = pg
                    .edge_endpoints(ei)
                    .expect("edge index obtained from edge_indices() must be valid");
                ResolvedEdge {
                    source: pg[src_idx].id,
                    target: pg[tgt_idx].id,
                    config: EdgeConfig { channel_buffer },
                }
            })
            .collect();

        Self { nodes, edges }
    }

    /// All nodes in topological order.
    pub fn nodes(&self) -> &[ResolvedNode] {
        &self.nodes
    }

    /// All edges with their channel configuration.
    pub fn edges(&self) -> &[ResolvedEdge] {
        &self.edges
    }
}

impl std::fmt::Debug for ExecutionPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionPlan")
            .field("nodes", &self.nodes.len())
            .field("edges", &self.edges.len())
            .finish()
    }
}

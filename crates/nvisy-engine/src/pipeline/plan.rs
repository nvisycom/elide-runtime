//! Graph compilation into an [`ExecutionPlan`].
//!
//! [`compile()`] bridges the user-facing [`Graph`] definition and the
//! runtime execution model. It performs these steps:
//!
//! 1. **Validation** — the graph is checked for structural correctness
//!    via [`Graph::validate`].
//! 2. **petgraph construction** — nodes and edges are inserted into a
//!    [`DiGraph`](petgraph::graph::DiGraph) for topological sorting.
//! 3. **Topological sort** — produces the node execution order, ensuring
//!    every node runs after its dependencies.
//! 4. **Policy resolution** — nodes without explicit retry/timeout
//!    policies inherit the engine-level defaults.
//! 5. **Edge resolution** — each edge is paired with an [`EdgeConfig`]
//!    controlling the bounded MPSC channel buffer size.
//!
//! The resulting [`ExecutionPlan`] is consumed by the
//! [orchestrator](super::orchestrator) to spawn concurrent tasks.

use nvisy_core::{Error, Result};
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use uuid::Uuid;

use crate::graph::{Graph, GraphEdge, GraphExt, GraphNode};

/// Channel configuration for a resolved edge.
#[derive(Debug, Clone)]
pub struct EdgeConfig {
    /// Buffer size for the bounded MPSC channel on this edge.
    pub channel_buffer: usize,
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

/// A graph node enriched with adjacency information.
///
/// The original [`GraphNode`] is preserved unchanged. Retry and timeout
/// policies are resolved at execution time by the [`NodeExecutor`]
/// (node-level policy falls back to engine defaults).
///
/// Order is implicit in the position within [`ExecutionPlan::nodes`].
///
/// [`NodeExecutor`]: super::executor::NodeExecutor
#[derive(Debug, Clone)]
pub struct ResolvedNode {
    /// The original graph node definition.
    pub node: GraphNode,
    /// IDs of nodes that feed data into this node.
    pub upstream_ids: Vec<Uuid>,
}

/// Compiles a [`Graph`] into an [`ExecutionPlan`].
///
/// Validates the graph, builds a petgraph representation, and
/// topologically sorts it. The graph is not mutated. Policy resolution
/// (retry/timeout defaults) happens later at execution time.
pub(crate) fn compile(graph: &Graph, channel_buffer: usize) -> Result<ExecutionPlan> {
    graph.validate()?;

    let pg = graph.to_petgraph();

    let topo =
        toposort(&pg, None).map_err(|_| Error::validation("graph contains a cycle", "compiler"))?;

    Ok(ExecutionPlan::from_graph(&pg, &topo, channel_buffer))
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

            nodes.push(ResolvedNode {
                node: graph_node.clone(),
                upstream_ids,
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

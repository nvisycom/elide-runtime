//! Compiled execution plan types and the `compile()` entry point.
//!
//! An [`ExecutionPlan`] is the central orchestration artifact produced by
//! [`compile()`]. It contains topologically-sorted [`ResolvedNode`]s and
//! pre-computed [`ResolvedEdge`]s with channel configuration.

mod edge;
mod node;

use nvisy_core::{Error, Result};
use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use uuid::Uuid;

pub use self::edge::{EdgeConfig, ResolvedEdge};
pub use self::node::ResolvedNode;
use super::policy::{CompiledRetryPolicy, CompiledTimeoutPolicy};
use crate::graph::policy::{RetryPolicy, TimeoutPolicy};
use crate::graph::{Graph, GraphEdge, GraphNode};

/// Compiles a [`Graph`] into an [`ExecutionPlan`].
///
/// Validates the graph, applies default policies to nodes that don't specify
/// their own, builds a petgraph representation, checks for cycles, and
/// produces a topologically-sorted plan.
pub(crate) fn compile(
    graph: &Graph,
    default_retry: Option<&RetryPolicy>,
    default_timeout: Option<&TimeoutPolicy>,
) -> Result<ExecutionPlan> {
    let mut graph = graph.clone();

    for node in &mut graph.nodes {
        if node.retry.is_none() {
            node.retry = default_retry.cloned();
        }
        if node.timeout.is_none() {
            node.timeout = default_timeout.cloned();
        }
    }

    graph.validate()?;

    let pg = build_petgraph(&graph)?;

    let topo =
        toposort(&pg, None).map_err(|_| Error::validation("graph contains a cycle", "compiler"))?;

    Ok(ExecutionPlan::from_graph(&pg, &topo))
}

/// Builds a petgraph `DiGraph` from a validated [`Graph`] and checks
/// for cycles.
fn build_petgraph(graph: &Graph) -> Result<DiGraph<GraphNode, GraphEdge>> {
    let mut pg = DiGraph::with_capacity(graph.nodes.len(), graph.edges.len());
    let mut index_map = std::collections::HashMap::with_capacity(graph.nodes.len());

    for node in &graph.nodes {
        let idx = pg.add_node(node.clone());
        index_map.insert(node.id, idx);
    }

    for edge in &graph.edges {
        let from = index_map[&edge.source];
        let to = index_map[&edge.target];
        pg.add_edge(from, to, edge.clone());
    }

    if is_cyclic_directed(&pg) {
        return Err(Error::validation("graph contains a cycle", "compiler"));
    }

    Ok(pg)
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
    fn from_graph(pg: &DiGraph<GraphNode, GraphEdge>, topo: &[NodeIndex]) -> Self {
        let mut nodes = Vec::with_capacity(topo.len());

        for &idx in topo {
            let graph_node = &pg[idx];
            let upstream_ids: Vec<Uuid> = pg
                .neighbors_directed(idx, petgraph::Direction::Incoming)
                .map(|n| pg[n].id)
                .collect();
            let downstream_ids: Vec<Uuid> = pg
                .neighbors_directed(idx, petgraph::Direction::Outgoing)
                .map(|n| pg[n].id)
                .collect();

            let compiled_retry = graph_node.retry().map(CompiledRetryPolicy::from);
            let compiled_timeout = graph_node.timeout().map(CompiledTimeoutPolicy::from);

            nodes.push(ResolvedNode {
                node: graph_node.clone(),
                upstream_ids,
                downstream_ids,
                compiled_retry,
                compiled_timeout,
            });
        }

        let edges: Vec<ResolvedEdge> = pg
            .edge_indices()
            .map(|ei| {
                let (src_idx, tgt_idx) = pg.edge_endpoints(ei).unwrap();
                ResolvedEdge {
                    source: pg[src_idx].id,
                    target: pg[tgt_idx].id,
                    config: EdgeConfig::default(),
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

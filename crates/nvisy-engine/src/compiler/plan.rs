//! Execution planning via topological sort.
//!
//! Converts a validated [`Graph`] into an [`ExecutionPlan`] by performing
//! cycle detection and topological sorting using `petgraph`.

use std::collections::HashMap;
use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use crate::compiler::graph::{Graph, GraphNode};
use nvisy_core::Error;

/// A graph node enriched with topological ordering and adjacency information.
#[derive(Debug, Clone)]
pub struct ResolvedNode {
    /// The original graph node definition.
    pub node: GraphNode,
    /// Zero-based position in the topological ordering.
    pub topo_order: usize,
    /// IDs of nodes that feed data into this node.
    pub upstream_ids: Vec<String>,
    /// IDs of nodes that receive data from this node.
    pub downstream_ids: Vec<String>,
}

/// A compiled execution plan ready for the executor.
///
/// Contains all nodes in topological order along with their adjacency
/// information so the executor can wire channels and schedule tasks.
pub struct ExecutionPlan {
    /// Resolved nodes sorted in topological order.
    pub nodes: Vec<ResolvedNode>,
    /// Node IDs in topological order.
    pub topo_order: Vec<String>,
}

/// Builds an execution plan from a parsed [`Graph`].
///
/// Validates that the graph is acyclic, performs a topological sort, and
/// computes upstream/downstream adjacency lists for each node.
///
/// Returns an error if the graph contains a cycle or references unknown nodes.
pub fn build_plan(graph: &Graph) -> Result<ExecutionPlan, Error> {
    // Build petgraph
    let mut pg: DiGraph<&str, ()> = DiGraph::new();
    let mut index_map: HashMap<&str, NodeIndex> = HashMap::new();

    for node in &graph.nodes {
        let idx = pg.add_node(node.id());
        index_map.insert(node.id(), idx);
    }

    for edge in &graph.edges {
        let from = index_map.get(edge.from.as_str()).ok_or_else(|| {
            Error::validation(format!("Unknown edge source: {}", edge.from), "compiler")
        })?;
        let to = index_map.get(edge.to.as_str()).ok_or_else(|| {
            Error::validation(format!("Unknown edge target: {}", edge.to), "compiler")
        })?;
        pg.add_edge(*from, *to, ());
    }

    // Cycle detection
    if is_cyclic_directed(&pg) {
        return Err(Error::validation("Graph contains a cycle", "compiler"));
    }

    // Topological sort
    let topo = toposort(&pg, None).map_err(|_| {
        Error::validation("Graph contains a cycle", "compiler")
    })?;

    let topo_order: Vec<String> = topo.iter().map(|idx| pg[*idx].to_string()).collect();

    // Build resolved nodes with adjacency info
    let node_map: HashMap<&str, &GraphNode> = graph.nodes.iter().map(|n| (n.id(), n)).collect();
    let mut resolved = Vec::new();

    for (order, node_id) in topo_order.iter().enumerate() {
        let node = node_map[node_id.as_str()];
        let idx = index_map[node_id.as_str()];

        let upstream_ids: Vec<String> = pg
            .neighbors_directed(idx, petgraph::Direction::Incoming)
            .map(|n| pg[n].to_string())
            .collect();

        let downstream_ids: Vec<String> = pg
            .neighbors_directed(idx, petgraph::Direction::Outgoing)
            .map(|n| pg[n].to_string())
            .collect();

        resolved.push(ResolvedNode {
            node: node.clone(),
            topo_order: order,
            upstream_ids,
            downstream_ids,
        });
    }

    Ok(ExecutionPlan {
        nodes: resolved,
        topo_order,
    })
}

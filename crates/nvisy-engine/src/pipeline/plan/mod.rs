//! Compiled execution plan types and the `compile()` entry point.
//!
//! An [`ExecutionPlan`] is the central orchestration artifact produced by
//! [`compile()`]. It contains topologically-sorted [`ResolvedNode`]s,
//! pre-computed adjacency information, [`ResolvedEdge`]s with channel
//! configuration, and [`PhaseGroup`]s for phase-aware scheduling.

mod edge;
mod node;
mod phase;

use std::collections::HashMap;

use nvisy_core::{Error, Result};
use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use uuid::Uuid;

pub use self::edge::{EdgeConfig, ResolvedEdge};
pub use self::node::ResolvedNode;
pub use self::phase::PhaseGroup;
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
    let mut index_map = HashMap::with_capacity(graph.nodes.len());

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
/// Contains all nodes in topological order, edges with channel configuration,
/// phase groupings, and pre-computed root/leaf indices. Constructed only via
/// [`compile()`].
pub struct ExecutionPlan {
    nodes: Vec<ResolvedNode>,
    edges: Vec<ResolvedEdge>,
    index_map: HashMap<Uuid, usize>,
    phases: Vec<PhaseGroup>,
    roots: Vec<usize>,
    leaves: Vec<usize>,
}

impl ExecutionPlan {
    /// Builds an execution plan from a petgraph and its topological ordering.
    fn from_graph(pg: &DiGraph<GraphNode, GraphEdge>, topo: &[NodeIndex]) -> Self {
        let mut index_map = HashMap::with_capacity(topo.len());
        let mut nodes = Vec::with_capacity(topo.len());

        for (i, &idx) in topo.iter().enumerate() {
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

            index_map.insert(graph_node.id, i);
            nodes.push(ResolvedNode {
                phase: graph_node.kind.phase(),
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

        let roots: Vec<usize> = nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.upstream_ids.is_empty())
            .map(|(i, _)| i)
            .collect();

        let leaves: Vec<usize> = nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.downstream_ids.is_empty())
            .map(|(i, _)| i)
            .collect();

        let mut phase_map: HashMap<u8, Vec<usize>> = HashMap::new();
        for (i, node) in nodes.iter().enumerate() {
            phase_map.entry(node.phase).or_default().push(i);
        }
        let mut phases: Vec<PhaseGroup> = phase_map
            .into_iter()
            .map(|(phase, node_indices)| PhaseGroup {
                phase,
                node_indices,
            })
            .collect();
        phases.sort_by_key(|g| g.phase);

        Self {
            nodes,
            edges,
            index_map,
            phases,
            roots,
            leaves,
        }
    }

    /// Number of nodes in the plan.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns `true` if the plan contains no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Number of edges in the plan.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// All nodes in topological order.
    pub fn nodes(&self) -> &[ResolvedNode] {
        &self.nodes
    }

    /// All edges with their channel configuration.
    pub fn edges(&self) -> &[ResolvedEdge] {
        &self.edges
    }

    /// Look up a node by its UUID in O(1).
    pub fn node_by_id(&self, id: Uuid) -> Option<&ResolvedNode> {
        self.index_map.get(&id).map(|&i| &self.nodes[i])
    }

    /// Returns the topological index for a node UUID.
    pub fn index_of(&self, id: Uuid) -> Option<usize> {
        self.index_map.get(&id).copied()
    }

    /// Indices of root nodes (no upstream dependencies).
    pub fn roots(&self) -> &[usize] {
        &self.roots
    }

    /// Indices of leaf nodes (no downstream dependents).
    pub fn leaves(&self) -> &[usize] {
        &self.leaves
    }

    /// Phase groups sorted by phase number, containing only occupied phases.
    pub fn phases(&self) -> &[PhaseGroup] {
        &self.phases
    }

    /// Iterator over edges originating from the given node.
    pub fn outgoing_edges(&self, id: Uuid) -> impl Iterator<Item = &ResolvedEdge> {
        self.edges.iter().filter(move |e| e.source == id)
    }

    /// Iterator over edges targeting the given node.
    pub fn incoming_edges(&self, id: Uuid) -> impl Iterator<Item = &ResolvedEdge> {
        self.edges.iter().filter(move |e| e.target == id)
    }
}

impl std::fmt::Debug for ExecutionPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionPlan")
            .field("nodes", &self.nodes.len())
            .field("edges", &self.edges.len())
            .field("phases", &self.phases.len())
            .field("roots", &self.roots)
            .field("leaves", &self.leaves)
            .finish()
    }
}

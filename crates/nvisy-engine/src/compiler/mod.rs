//! Pipeline compilation: graph construction, validation, and execution planning.
//!
//! The [`Compiler`] is the entry-point for turning a [`Graph`] into an
//! [`ExecutionPlan`]. It carries optional default retry and timeout policies
//! that are applied to nodes which don't specify their own.

mod graph;
mod plan;
mod policy;

use std::collections::HashMap;

use nvisy_core::Error;
use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use uuid::Uuid;

pub use self::graph::{
    ActionKind, ActionNode, Graph, GraphEdge, GraphNode, GraphNodeKind, SourceNode, TargetNode,
};
pub(crate) use self::plan::{ExecutionPlan, ResolvedNode};
pub use self::policy::{BackoffStrategy, RetryPolicy, TimeoutBehavior, TimeoutPolicy};

/// Pipeline compiler with optional default policies.
///
/// Nodes that don't carry their own retry or timeout policy will inherit
/// the compiler-level defaults (if set) at compile time.
#[derive(Debug, Clone, Default)]
pub(crate) struct Compiler {
    /// Default retry policy applied to nodes without one.
    pub retry: Option<RetryPolicy>,
    /// Default timeout policy applied to nodes without one.
    pub timeout: Option<TimeoutPolicy>,
}

impl Compiler {
    /// Create a compiler with no default policies.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the default retry policy.
    pub fn with_retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = Some(policy);
        self
    }

    /// Set the default timeout policy.
    pub fn with_timeout(mut self, policy: TimeoutPolicy) -> Self {
        self.timeout = Some(policy);
        self
    }

    /// Compile a [`Graph`] into an [`ExecutionPlan`].
    ///
    /// Validates the graph, applies compiler-level default policies to nodes
    /// that don't specify their own, builds a `petgraph` representation,
    /// checks for cycles, and produces a topologically-sorted plan.
    pub fn compile(&self, graph: &Graph) -> Result<ExecutionPlan, Error> {
        let mut graph = graph.clone();

        // Apply compiler-level defaults to nodes missing their own policies.
        for node in &mut graph.nodes {
            if node.retry.is_none() {
                node.retry.clone_from(&self.retry);
            }
            if node.timeout.is_none() {
                node.timeout.clone_from(&self.timeout);
            }
        }

        graph.validate()?;

        // Build petgraph
        let mut pg: DiGraph<GraphNode, ()> = DiGraph::new();
        let mut index_map: HashMap<Uuid, NodeIndex> = HashMap::new();

        for node in &graph.nodes {
            let idx = pg.add_node(node.clone());
            index_map.insert(node.id, idx);
        }

        for edge in &graph.edges {
            let from = index_map[&edge.source];
            let to = index_map[&edge.target];
            pg.add_edge(from, to, ());
        }

        // Cycle detection
        if is_cyclic_directed(&pg) {
            return Err(Error::validation("Graph contains a cycle", "compiler"));
        }

        // Topological sort
        let topo = toposort(&pg, None)
            .map_err(|_| Error::validation("Graph contains a cycle", "compiler"))?;

        // Build resolved nodes with adjacency info in topological order.
        let mut resolved = Vec::new();

        for idx in &topo {
            let upstream_ids: Vec<Uuid> = pg
                .neighbors_directed(*idx, petgraph::Direction::Incoming)
                .map(|n| pg[n].id)
                .collect();

            let downstream_ids: Vec<Uuid> = pg
                .neighbors_directed(*idx, petgraph::Direction::Outgoing)
                .map(|n| pg[n].id)
                .collect();

            resolved.push(ResolvedNode {
                node: pg[*idx].clone(),
                upstream_ids,
                downstream_ids,
            });
        }

        Ok(ExecutionPlan { nodes: resolved })
    }
}

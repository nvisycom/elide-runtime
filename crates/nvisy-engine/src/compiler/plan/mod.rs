//! Compiled execution plan types.
//!
//! A [`CompiledGraph`] wraps a `petgraph` representation of the pipeline.
//! An [`ExecutionPlan`] pairs it with topologically-sorted [`ResolvedNode`]s
//! so the executor can wire channels and schedule tasks.

use std::collections::HashMap;

use petgraph::graph::{DiGraph, NodeIndex};
use uuid::Uuid;

use crate::compiler::graph::GraphNode;

/// A compiled graph with petgraph representation, ready for execution.
#[derive(Debug, Clone)]
pub struct CompiledGraph {
    /// petgraph directed graph: node weight is the `GraphNode`, edge weight is `()`.
    pub graph: DiGraph<GraphNode, ()>,
    /// Lookup from node UUID to petgraph `NodeIndex`.
    pub index_map: HashMap<Uuid, NodeIndex>,
}

/// A graph node enriched with topological ordering and adjacency information.
#[derive(Debug, Clone)]
pub struct ResolvedNode {
    /// The original graph node definition.
    pub node: GraphNode,
    /// Zero-based position in the topological ordering.
    pub topo_order: usize,
    /// IDs of nodes that feed data into this node.
    pub upstream_ids: Vec<Uuid>,
    /// IDs of nodes that receive data from this node.
    pub downstream_ids: Vec<Uuid>,
}

/// A compiled execution plan ready for the executor.
///
/// Contains all nodes in topological order along with their adjacency
/// information so the executor can wire channels and schedule tasks.
pub struct ExecutionPlan {
    /// Resolved nodes sorted in topological order.
    pub nodes: Vec<ResolvedNode>,
    /// Node IDs in topological order.
    pub topo_order: Vec<Uuid>,
    /// The compiled petgraph representation.
    pub compiled: CompiledGraph,
}

//! Compiled execution plan types.
//!
//! An [`ExecutionPlan`] contains topologically-sorted [`ResolvedNode`]s
//! so the executor can wire channels and schedule tasks.

use uuid::Uuid;

use crate::compiler::graph::GraphNode;

/// A graph node enriched with adjacency information.
///
/// Order is implicit in the position within [`ExecutionPlan::nodes`].
#[derive(Debug, Clone)]
pub struct ResolvedNode {
    /// The original graph node definition.
    pub node: GraphNode,
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
}

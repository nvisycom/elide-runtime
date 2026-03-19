//! Resolved node type for the compiled execution plan.

use uuid::Uuid;

use crate::graph::{GraphNode, RetryPolicy, TimeoutPolicy};

/// A graph node enriched with adjacency information and compiled policies.
///
/// Order is implicit in the position within [`ExecutionPlan::nodes`].
///
/// [`ExecutionPlan::nodes`]: super::ExecutionPlan::nodes
#[derive(Debug, Clone)]
pub struct ResolvedNode {
    /// The original graph node definition.
    pub node: GraphNode,
    /// IDs of nodes that feed data into this node.
    pub upstream_ids: Vec<Uuid>,
    /// IDs of nodes that receive data from this node.
    pub downstream_ids: Vec<Uuid>,
    /// Retry policy for this node, if configured.
    pub retry: Option<RetryPolicy>,
    /// Timeout policy for this node, if configured.
    pub timeout: Option<TimeoutPolicy>,
}

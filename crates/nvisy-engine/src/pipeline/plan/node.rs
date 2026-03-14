//! Resolved node type for the compiled execution plan.

use uuid::Uuid;

use crate::graph::GraphNode;
use crate::pipeline::policy::{CompiledRetryPolicy, CompiledTimeoutPolicy};

/// A graph node enriched with adjacency information and compiled policies.
///
/// Order is implicit in the position within [`ExecutionPlan::nodes`](super::ExecutionPlan::nodes).
#[derive(Debug, Clone)]
pub struct ResolvedNode {
    /// The original graph node definition.
    pub node: GraphNode,
    /// Pipeline phase for this node (derived from the node kind).
    pub phase: u8,
    /// IDs of nodes that feed data into this node.
    pub upstream_ids: Vec<Uuid>,
    /// IDs of nodes that receive data from this node.
    pub downstream_ids: Vec<Uuid>,
    /// Pre-compiled retry policy, if configured.
    pub compiled_retry: Option<CompiledRetryPolicy>,
    /// Pre-compiled timeout policy, if configured.
    pub compiled_timeout: Option<CompiledTimeoutPolicy>,
}

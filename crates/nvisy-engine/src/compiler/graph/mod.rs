//! Graph data model for pipeline definitions.
//!
//! A pipeline is represented as a set of [`GraphNode`]s connected by
//! [`GraphEdge`]s, collected into a [`Graph`]. Nodes are flattened into
//! a struct carrying shared fields (`id`, `retry`, `timeout`) alongside
//! a `kind` discriminator that determines the node's role.

mod action;
mod retry;
mod source;
mod target;
mod timeout;

pub use action::{ActionKind, ActionNode};
pub use retry::{BackoffStrategy, RetryPolicy};
pub use source::SourceNode;
pub use target::TargetNode;
pub use timeout::{TimeoutBehavior, TimeoutPolicy};

use std::collections::HashSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nvisy_core::Error;


/// A node in the pipeline graph.
///
/// Shared fields (`id`, `retry`, `timeout`) live directly on the struct
/// while the role-specific payload is carried in [`GraphNodeKind`] via
/// `#[serde(flatten)]`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GraphNode {
    /// Unique identifier for this node within the graph.
    pub id: Uuid,
    /// Optional retry policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryPolicy>,
    /// Optional timeout policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<TimeoutPolicy>,
    /// Role-specific payload (source, action, or target).
    #[serde(flatten)]
    pub kind: GraphNodeKind,
}

impl GraphNode {
    /// Returns the retry policy, if one is configured.
    pub fn retry(&self) -> Option<&RetryPolicy> {
        self.retry.as_ref()
    }

    /// Returns the timeout policy, if one is configured.
    pub fn timeout(&self) -> Option<&TimeoutPolicy> {
        self.timeout.as_ref()
    }
}

/// Discriminator for the three node roles in a pipeline.
///
/// Serialized with a `"type"` tag so JSON definitions specify
/// `"source"`, `"action"`, or `"target"`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraphNodeKind {
    /// A data source that reads from an external provider via a named stream.
    Source(SourceNode),
    /// A transformation or detection step applied to data flowing through the pipeline.
    Action(ActionNode),
    /// A data sink that writes to an external provider via a named stream.
    Target(TargetNode),
}

/// A directed edge connecting two nodes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GraphEdge {
    /// ID of the upstream node.
    pub source: Uuid,
    /// ID of the downstream node.
    pub target: Uuid,
}

/// A complete pipeline graph definition containing nodes and edges.
///
/// The graph must be a valid DAG (directed acyclic graph) with unique node IDs.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Graph {
    /// All nodes in the pipeline.
    pub nodes: Vec<GraphNode>,
    /// Directed edges describing data flow between nodes.
    pub edges: Vec<GraphEdge>,
}

impl Graph {
    /// Validate structural invariants.
    ///
    /// - The graph must contain at least one node.
    /// - All node IDs must be unique.
    /// - All edge endpoints must reference existing node IDs.
    pub fn validate(&self) -> Result<(), Error> {
        if self.nodes.is_empty() {
            return Err(Error::validation("Graph must have at least one node", "compiler"));
        }

        let mut seen = HashSet::new();
        for node in &self.nodes {
            if !seen.insert(node.id) {
                return Err(Error::validation(
                    format!("Duplicate node ID: {}", node.id),
                    "compiler",
                ));
            }
        }

        let node_ids: HashSet<Uuid> = seen;
        for edge in &self.edges {
            if !node_ids.contains(&edge.source) {
                return Err(Error::validation(
                    format!("Edge references unknown source node: {}", edge.source),
                    "compiler",
                ));
            }
            if !node_ids.contains(&edge.target) {
                return Err(Error::validation(
                    format!("Edge references unknown target node: {}", edge.target),
                    "compiler",
                ));
            }
        }

        Ok(())
    }
}

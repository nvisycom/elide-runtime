//! Graph data model for pipeline definitions.
//!
//! A pipeline is represented as a set of [`GraphNode`]s connected by
//! [`GraphEdge`]s, collected into a [`Graph`].

use serde::{Deserialize, Serialize};

use crate::policies::retry::RetryPolicy;

/// A node in the pipeline graph, tagged by its role.
///
/// Nodes are serialized with a `"type"` discriminator so JSON definitions
/// can specify `"source"`, `"action"`, or `"target"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraphNode {
    /// A data source that reads from an external provider via a named stream.
    Source {
        /// Unique identifier for this node within the graph.
        id: String,
        /// Provider name used to resolve the connection (e.g. `"s3"`).
        provider: String,
        /// Stream name on the provider (e.g. `"read"`).
        stream: String,
        /// Arbitrary provider-specific parameters.
        #[serde(default)]
        params: serde_json::Value,
        /// Optional retry policy applied to this node's execution.
        #[serde(skip_serializing_if = "Option::is_none")]
        retry: Option<RetryPolicy>,
        /// Optional per-node timeout in milliseconds.
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    /// A transformation or detection step applied to data flowing through the pipeline.
    Action {
        /// Unique identifier for this node within the graph.
        id: String,
        /// Registered action name (e.g. `"detect_regex"`, `"classify"`).
        action: String,
        /// Arbitrary action-specific parameters.
        #[serde(default)]
        params: serde_json::Value,
        /// Optional retry policy applied to this node's execution.
        #[serde(skip_serializing_if = "Option::is_none")]
        retry: Option<RetryPolicy>,
        /// Optional per-node timeout in milliseconds.
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    /// A data sink that writes to an external provider via a named stream.
    Target {
        /// Unique identifier for this node within the graph.
        id: String,
        /// Provider name used to resolve the connection (e.g. `"s3"`).
        provider: String,
        /// Stream name on the provider (e.g. `"write"`).
        stream: String,
        /// Arbitrary provider-specific parameters.
        #[serde(default)]
        params: serde_json::Value,
        /// Optional retry policy applied to this node's execution.
        #[serde(skip_serializing_if = "Option::is_none")]
        retry: Option<RetryPolicy>,
        /// Optional per-node timeout in milliseconds.
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
}

impl GraphNode {
    /// Returns the unique identifier shared by all node variants.
    pub fn id(&self) -> &str {
        match self {
            GraphNode::Source { id, .. } => id,
            GraphNode::Action { id, .. } => id,
            GraphNode::Target { id, .. } => id,
        }
    }

    /// Returns the parameters value for this node.
    pub fn params(&self) -> &serde_json::Value {
        match self {
            GraphNode::Source { params, .. } => params,
            GraphNode::Action { params, .. } => params,
            GraphNode::Target { params, .. } => params,
        }
    }

    /// Returns the retry policy, if one is configured.
    pub fn retry(&self) -> Option<&RetryPolicy> {
        match self {
            GraphNode::Source { retry, .. } => retry.as_ref(),
            GraphNode::Action { retry, .. } => retry.as_ref(),
            GraphNode::Target { retry, .. } => retry.as_ref(),
        }
    }

    /// Returns the per-node timeout in milliseconds, if one is configured.
    pub fn timeout_ms(&self) -> Option<u64> {
        match self {
            GraphNode::Source { timeout_ms, .. } => *timeout_ms,
            GraphNode::Action { timeout_ms, .. } => *timeout_ms,
            GraphNode::Target { timeout_ms, .. } => *timeout_ms,
        }
    }
}

/// A directed edge connecting two nodes by their IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct GraphEdge {
    /// ID of the upstream (source) node.
    pub from: String,
    /// ID of the downstream (destination) node.
    pub to: String,
}

/// A complete pipeline graph definition containing nodes and edges.
///
/// The graph must be a valid DAG (directed acyclic graph) with unique node IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Graph {
    /// All nodes in the pipeline.
    pub nodes: Vec<GraphNode>,
    /// Directed edges describing data flow between nodes.
    pub edges: Vec<GraphEdge>,
}

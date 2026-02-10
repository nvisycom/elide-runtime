use serde::{Deserialize, Serialize};

/// Retry policy for a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RetryPolicy {
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_delay_ms")]
    pub delay_ms: u64,
    #[serde(default)]
    pub backoff: BackoffStrategy,
}

fn default_max_retries() -> u32 { 3 }
fn default_delay_ms() -> u64 { 1000 }

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            delay_ms: 1000,
            backoff: BackoffStrategy::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BackoffStrategy {
    #[default]
    Fixed,
    Exponential,
    Jitter,
}

/// A node in the graph definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraphNode {
    Source {
        id: String,
        provider: String,
        stream: String,
        #[serde(default)]
        params: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        retry: Option<RetryPolicy>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    Action {
        id: String,
        action: String,
        #[serde(default)]
        params: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        retry: Option<RetryPolicy>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    Target {
        id: String,
        provider: String,
        stream: String,
        #[serde(default)]
        params: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        retry: Option<RetryPolicy>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
}

impl GraphNode {
    pub fn id(&self) -> &str {
        match self {
            GraphNode::Source { id, .. } => id,
            GraphNode::Action { id, .. } => id,
            GraphNode::Target { id, .. } => id,
        }
    }

    pub fn params(&self) -> &serde_json::Value {
        match self {
            GraphNode::Source { params, .. } => params,
            GraphNode::Action { params, .. } => params,
            GraphNode::Target { params, .. } => params,
        }
    }

    pub fn retry(&self) -> Option<&RetryPolicy> {
        match self {
            GraphNode::Source { retry, .. } => retry.as_ref(),
            GraphNode::Action { retry, .. } => retry.as_ref(),
            GraphNode::Target { retry, .. } => retry.as_ref(),
        }
    }

    pub fn timeout_ms(&self) -> Option<u64> {
        match self {
            GraphNode::Source { timeout_ms, .. } => *timeout_ms,
            GraphNode::Action { timeout_ms, .. } => *timeout_ms,
            GraphNode::Target { timeout_ms, .. } => *timeout_ms,
        }
    }
}

/// An edge connecting two nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
}

/// A complete graph definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

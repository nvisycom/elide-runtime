//! Pipeline run lifecycle types and in-memory storage.
//!
//! A run progresses through [`RunStatus`] states: `Pending` → `Running` →
//! `Succeeded` | `PartialFailure` | `Failed` | `Cancelled`. Each node
//! within a run tracks its own [`NodeStatus`].
//!
//! Two projection types serve different API needs:
//!
//! - [`RunSnapshot`] — full detail including per-node snapshots
//!   (`GET /runs/{id}`).
//! - [`RunSummary`] — lightweight with node count only, no per-node
//!   detail (`GET /runs`).
//!
//! The [`state`] submodule contains the volatile in-memory storage
//! ([`RunState`](state::RunState)) backing all run queries and mutations.

mod analytics;
pub(crate) mod state;

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::analytics::AnalyticsSnapshot;

/// Lifecycle status of a pipeline run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// The run has been created but not yet started.
    Pending,
    /// The run is actively executing nodes.
    Running,
    /// All nodes completed without error.
    Succeeded,
    /// Some nodes succeeded while others failed.
    PartialFailure,
    /// All nodes failed.
    Failed,
    /// The run was cancelled by the caller.
    Cancelled,
}

/// Lifecycle status of a single node within a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    /// The node is waiting for upstream dependencies.
    Pending,
    /// The node is actively executing.
    Running,
    /// The node completed without error.
    Succeeded,
    /// The node failed.
    Failed,
}

/// Point-in-time snapshot of a single node within a run.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodeSnapshot {
    /// ID of the node.
    pub node_id: Uuid,
    /// Current status of this node.
    pub status: NodeStatus,
    /// Number of data items processed so far.
    pub items_processed: u64,
    /// Error message if the node failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Full point-in-time snapshot of a pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RunSnapshot {
    /// Unique run identifier.
    pub id: Uuid,
    /// Identity of the actor who initiated the run.
    pub actor_id: Uuid,
    /// Current overall status.
    pub status: RunStatus,
    /// Timestamp when the run was created.
    #[schemars(with = "String")]
    pub created_at: Timestamp,
    /// Timestamp when execution actually started, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub started_at: Option<Timestamp>,
    /// Timestamp when the run finished, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub completed_at: Option<Timestamp>,
    /// Per-node snapshots.
    pub nodes: Vec<NodeSnapshot>,
}

/// Lightweight summary of a run for listing endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RunSummary {
    /// Unique run identifier.
    pub id: Uuid,
    /// Identity of the actor who initiated the run.
    pub actor_id: Uuid,
    /// Current overall status.
    pub status: RunStatus,
    /// Timestamp when the run was created.
    #[schemars(with = "String")]
    pub created_at: Timestamp,
    /// Timestamp when execution actually started, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub started_at: Option<Timestamp>,
    /// Timestamp when the run finished, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub completed_at: Option<Timestamp>,
    /// Number of nodes in the execution graph.
    pub node_count: usize,
}

/// Filter criteria for listing runs.
#[derive(Debug, Clone, Default)]
pub struct RunFilter {
    /// If set, only return runs with this status.
    pub status: Option<RunStatus>,
}

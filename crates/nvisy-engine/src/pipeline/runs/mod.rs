//! Pipeline run data types, state storage, and the [`EngineRuns`] trait.
//!
//! Pure data definitions for run lifecycle tracking. All mutation and
//! querying happens through the [`EngineRuns`] trait, implemented on
//! [`DefaultEngine`].
//!
//! [`DefaultEngine`]: super::DefaultEngine

pub(crate) mod state;

use std::future::Future;

use jiff::Timestamp;
use nvisy_core::Error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

/// Read-only access to pipeline run state.
///
/// Runs are created internally by [`Engine::run()`].
///
/// [`Engine::run()`]: super::Engine::run
/// External callers can inspect and cancel runs through this trait.
pub trait EngineRuns: Send + Sync {
    /// Get a full snapshot of a single run.
    fn get_run(&self, actor_id: Uuid, id: Uuid)
    -> impl Future<Output = Option<RunSnapshot>> + Send;

    /// List runs matching the given filter.
    fn list_runs(
        &self,
        actor_id: Uuid,
        filter: RunFilter,
    ) -> impl Future<Output = Vec<RunSummary>> + Send;

    /// Request cancellation of an in-progress run.
    ///
    /// Returns `Err` if the run was not found or has already finished.
    fn cancel_run(
        &self,
        actor_id: Uuid,
        id: Uuid,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Delete a single finished run.
    ///
    /// Returns `Err` if the run does not exist or is still active.
    fn delete_run(
        &self,
        actor_id: Uuid,
        id: Uuid,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Delete all finished runs. Returns the number of removed entries.
    fn delete_all_runs(&self, actor_id: Uuid) -> impl Future<Output = usize> + Send;
}

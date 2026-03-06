//! Pipeline run lifecycle management.
//!
//! Tracks the status of every pipeline execution from creation through
//! completion or cancellation. Provides [`RunManager`] for concurrent
//! read/write access to run state.

use std::collections::HashMap;
use std::sync::Arc;

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::executor::{NodeOutput, RunOutput};

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
    Success,
    /// Some nodes succeeded while others failed.
    PartialFailure,
    /// All nodes failed.
    Failure,
    /// The run was cancelled by the caller.
    Cancelled,
}

/// Execution progress of a single node within a run.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodeProgress {
    /// ID of the node this progress belongs to.
    pub node_id: Uuid,
    /// Current status of this node.
    pub status: RunStatus,
    /// Number of data items processed so far.
    pub items_processed: u64,
    /// Error message if the node failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl From<&NodeOutput> for NodeProgress {
    fn from(nr: &NodeOutput) -> Self {
        Self {
            node_id: nr.node_id,
            status: if nr.error.is_none() {
                RunStatus::Success
            } else {
                RunStatus::Failure
            },
            items_processed: nr.items_processed,
            error: nr.error.clone(),
        }
    }
}

/// Complete mutable state of a pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RunState {
    /// Unique run identifier.
    pub id: Uuid,
    /// Current overall status.
    pub status: RunStatus,
    /// Timestamp when the run was created.
    #[schemars(with = "String")]
    pub created_at: Timestamp,
    /// Timestamp when the run finished, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub completed_at: Option<Timestamp>,
    /// Per-node progress keyed by node ID.
    pub node_progress: HashMap<Uuid, NodeProgress>,
    /// Final result after the run completes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<RunOutput>,
}

/// Lightweight summary of a run for listing endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RunSummary {
    /// Unique run identifier.
    pub id: Uuid,
    /// Current overall status.
    pub status: RunStatus,
    /// Timestamp when the run was created.
    #[schemars(with = "String")]
    pub created_at: Timestamp,
    /// Timestamp when the run finished, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub completed_at: Option<Timestamp>,
}

/// Thread-safe manager that tracks all pipeline runs.
///
/// Internally uses [`RwLock`]-protected maps so multiple readers can inspect
/// run state concurrently while writes are serialized.
pub struct RunManager {
    /// All known runs keyed by their UUID.
    runs: Arc<RwLock<HashMap<Uuid, RunState>>>,
    /// Cancellation tokens for runs that are still in progress.
    cancel_tokens: Arc<RwLock<HashMap<Uuid, CancellationToken>>>,
}

impl RunManager {
    /// Creates a new, empty run manager.
    pub fn new() -> Self {
        Self {
            runs: Arc::new(RwLock::new(HashMap::new())),
            cancel_tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new pending run and return its ID and cancellation token.
    pub async fn create_run(&self) -> (Uuid, CancellationToken) {
        let id = Uuid::new_v4();
        let token = CancellationToken::new();

        let state = RunState {
            id,
            status: RunStatus::Pending,
            created_at: Timestamp::now(),
            completed_at: None,
            node_progress: HashMap::new(),
            result: None,
        };

        self.runs.write().await.insert(id, state);
        self.cancel_tokens.write().await.insert(id, token.clone());

        (id, token)
    }

    /// Update a run to running status.
    pub async fn set_running(&self, id: Uuid) {
        if let Some(state) = self.runs.write().await.get_mut(&id) {
            state.status = RunStatus::Running;
        }
    }

    /// Complete a run with a result.
    pub async fn complete_run(&self, id: Uuid, result: RunOutput) {
        if let Some(state) = self.runs.write().await.get_mut(&id) {
            state.status = if result.success {
                RunStatus::Success
            } else if result.node_results.iter().any(|r| r.error.is_none()) {
                RunStatus::PartialFailure
            } else {
                RunStatus::Failure
            };
            state.completed_at = Some(Timestamp::now());

            for nr in &result.node_results {
                state
                    .node_progress
                    .insert(nr.node_id, NodeProgress::from(nr));
            }

            state.result = Some(result);
        }
        self.cancel_tokens.write().await.remove(&id);
    }

    /// Get the current state of a run.
    pub async fn get(&self, id: Uuid) -> Option<RunState> {
        self.runs.read().await.get(&id).cloned()
    }

    /// List all runs, optionally filtered by status.
    pub async fn list(&self, status: Option<RunStatus>) -> Vec<RunSummary> {
        self.runs
            .read()
            .await
            .values()
            .filter(|s| status.is_none_or(|st| s.status == st))
            .map(|s| RunSummary {
                id: s.id,
                status: s.status,
                created_at: s.created_at,
                completed_at: s.completed_at,
            })
            .collect()
    }

    /// Cancel a running or pending run. Returns false if not found or already finished.
    pub async fn cancel(&self, id: Uuid) -> bool {
        if let Some(token) = self.cancel_tokens.read().await.get(&id) {
            token.cancel();
            if let Some(state) = self.runs.write().await.get_mut(&id) {
                state.status = RunStatus::Cancelled;
                state.completed_at = Some(Timestamp::now());
            }
            true
        } else {
            false
        }
    }
}

impl Default for RunManager {
    fn default() -> Self {
        Self::new()
    }
}

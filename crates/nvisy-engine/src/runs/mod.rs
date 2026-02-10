use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use crate::executor::runner::RunResult;

/// Status of a pipeline run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    Success,
    PartialFailure,
    Failure,
    Cancelled,
}

/// Progress of a single node within a run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct NodeProgress {
    pub node_id: String,
    pub status: RunStatus,
    pub items_processed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Full state of a run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RunState {
    pub id: Uuid,
    pub status: RunStatus,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    pub node_progress: HashMap<String, NodeProgress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<RunResult>,
}

/// Summary of a run for listing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RunSummary {
    pub id: Uuid,
    pub status: RunStatus,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
}

/// Manages all tracked runs.
pub struct RunManager {
    runs: Arc<RwLock<HashMap<Uuid, RunState>>>,
    cancel_tokens: Arc<RwLock<HashMap<Uuid, CancellationToken>>>,
}

impl RunManager {
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
            created_at: Utc::now(),
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
    pub async fn complete_run(&self, id: Uuid, result: RunResult) {
        if let Some(state) = self.runs.write().await.get_mut(&id) {
            state.status = if result.success {
                RunStatus::Success
            } else if result.node_results.iter().any(|r| r.error.is_none()) {
                RunStatus::PartialFailure
            } else {
                RunStatus::Failure
            };
            state.completed_at = Some(Utc::now());

            for nr in &result.node_results {
                state.node_progress.insert(
                    nr.node_id.clone(),
                    NodeProgress {
                        node_id: nr.node_id.clone(),
                        status: if nr.error.is_none() {
                            RunStatus::Success
                        } else {
                            RunStatus::Failure
                        },
                        items_processed: nr.items_processed,
                        error: nr.error.clone(),
                    },
                );
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
                state.completed_at = Some(Utc::now());
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

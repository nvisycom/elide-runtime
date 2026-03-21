//! In-memory run state storage.
//!
//! [`RunState`] wraps a concurrent map of [`RunEntry`] records,
//! providing the read/write operations needed by the engine and
//! orchestrator. All runs are lost on restart — this is an in-memory
//! implementation.

use std::collections::HashMap;
use std::sync::Arc;

use jiff::Timestamp;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{NodeSnapshot, NodeStatus, RunFilter, RunSnapshot, RunStatus, RunSummary};
use crate::pipeline::analytics::AnalyticsSnapshot;

/// In-memory run state shared across engine clones.
#[derive(Clone)]
pub(crate) struct RunState {
    runs: Arc<RwLock<HashMap<Uuid, RunEntry>>>,
}

/// Private mutable state for a single run.
pub(crate) struct RunEntry {
    pub actor_id: Uuid,
    pub status: RunStatus,
    pub created_at: Timestamp,
    pub completed_at: Option<Timestamp>,
    pub nodes: HashMap<Uuid, NodeSnapshot>,
    pub cancel: CancellationToken,
    pub entities_detected: u64,
    pub redactions_applied: u64,
}

impl RunEntry {
    fn to_snapshot(&self, id: Uuid) -> RunSnapshot {
        RunSnapshot {
            id,
            actor_id: self.actor_id,
            status: self.status,
            created_at: self.created_at,
            completed_at: self.completed_at,
            nodes: self.nodes.values().cloned().collect(),
        }
    }

    fn to_summary(&self, id: Uuid) -> RunSummary {
        RunSummary {
            id,
            actor_id: self.actor_id,
            status: self.status,
            created_at: self.created_at,
            completed_at: self.completed_at,
            node_count: self.nodes.len(),
        }
    }
}

impl RunState {
    pub fn new() -> Self {
        Self {
            runs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Insert a new run entry.
    pub async fn insert(&self, run_id: Uuid, entry: RunEntry) {
        self.runs.write().await.insert(run_id, entry);
    }

    /// Update a single node's snapshot within a run.
    pub async fn update_node(
        &self,
        run_id: Uuid,
        node_id: Uuid,
        status: NodeStatus,
        items_processed: u64,
        error: Option<String>,
    ) {
        if let Some(entry) = self.runs.write().await.get_mut(&run_id)
            && let Some(node) = entry.nodes.get_mut(&node_id)
        {
            node.status = status;
            node.items_processed = items_processed;
            node.error = error;
        }
    }

    /// Transition a run to its final status.
    pub async fn finalize(
        &self,
        run_id: Uuid,
        status: RunStatus,
        entities_detected: u64,
        redactions_applied: u64,
    ) {
        if let Some(entry) = self.runs.write().await.get_mut(&run_id) {
            entry.status = status;
            entry.completed_at = Some(Timestamp::now());
            entry.entities_detected = entities_detected;
            entry.redactions_applied = redactions_applied;
        }
    }

    /// Mark a run as failed without entity/redaction counts.
    pub async fn fail(&self, run_id: Uuid) {
        self.finalize(run_id, RunStatus::Failed, 0, 0).await;
    }

    /// Get a full snapshot of a single run.
    pub async fn get_run(&self, id: Uuid) -> Option<RunSnapshot> {
        self.runs
            .read()
            .await
            .get(&id)
            .map(|entry| entry.to_snapshot(id))
    }

    /// List runs matching the given filter.
    pub async fn list_runs(&self, filter: &RunFilter) -> Vec<RunSummary> {
        self.runs
            .read()
            .await
            .iter()
            .filter(|(_, entry)| {
                filter.status.is_none_or(|s| entry.status == s)
                    && filter.actor_id.is_none_or(|a| entry.actor_id == a)
            })
            .map(|(&id, entry)| entry.to_summary(id))
            .collect()
    }

    /// Request cancellation of an in-progress run.
    pub async fn cancel_run(&self, id: Uuid) -> Result<(), nvisy_core::Error> {
        let mut runs = self.runs.write().await;
        let entry = runs.get_mut(&id).ok_or_else(|| {
            nvisy_core::Error::new(nvisy_core::ErrorKind::NotFound, "run not found")
        })?;

        match entry.status {
            RunStatus::Pending | RunStatus::Running => {
                entry.cancel.cancel();
                entry.status = RunStatus::Cancelled;
                entry.completed_at = Some(Timestamp::now());
                Ok(())
            }
            _ => Err(nvisy_core::Error::new(
                nvisy_core::ErrorKind::Validation,
                "run has already finished",
            )
            .with_component("run")),
        }
    }

    /// Remove a single run from the store.
    ///
    /// Returns `Err` if the run does not exist or is still active.
    pub async fn delete_run(&self, id: Uuid) -> Result<(), nvisy_core::Error> {
        let mut runs = self.runs.write().await;
        let entry = runs.get(&id).ok_or_else(|| {
            nvisy_core::Error::new(nvisy_core::ErrorKind::NotFound, "run not found")
        })?;

        match entry.status {
            RunStatus::Pending | RunStatus::Running => {
                return Err(nvisy_core::Error::new(
                    nvisy_core::ErrorKind::Validation,
                    "cannot delete an active run",
                )
                .with_component("run"));
            }
            _ => {}
        }

        runs.remove(&id);
        Ok(())
    }

    /// Remove all finished runs from the store.
    ///
    /// Active runs (pending or running) are preserved. Returns the
    /// number of removed entries.
    pub async fn delete_all_runs(&self) -> usize {
        let mut runs = self.runs.write().await;
        let before = runs.len();
        runs.retain(|_, entry| matches!(entry.status, RunStatus::Pending | RunStatus::Running));
        before - runs.len()
    }

    /// Collect a point-in-time analytics snapshot.
    pub async fn snapshot(&self) -> AnalyticsSnapshot {
        let runs = self.runs.read().await;
        let mut active = 0u64;
        let mut succeeded = 0u64;
        let mut failed = 0u64;
        let mut cancelled = 0u64;
        let mut total_entities = 0u64;
        let mut total_redactions = 0u64;
        let mut actors = std::collections::HashSet::new();

        for entry in runs.values() {
            match entry.status {
                RunStatus::Pending | RunStatus::Running => active += 1,
                RunStatus::Succeeded => succeeded += 1,
                RunStatus::Failed | RunStatus::PartialFailure => failed += 1,
                RunStatus::Cancelled => cancelled += 1,
            }
            actors.insert(entry.actor_id);
            total_entities += entry.entities_detected;
            total_redactions += entry.redactions_applied;
        }

        AnalyticsSnapshot {
            timestamp: Timestamp::now(),
            total_runs: runs.len() as u64,
            active_runs: active,
            succeeded_runs: succeeded,
            failed_runs: failed,
            cancelled_runs: cancelled,
            total_entities_detected: total_entities,
            total_redactions_applied: total_redactions,
            distinct_actors: actors.len() as u64,
        }
    }
}

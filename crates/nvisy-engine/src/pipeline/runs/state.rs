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

/// In-memory run state.
///
/// Cheaply clonable (`Arc` bump). All clones share the same
/// underlying data.
#[derive(Clone)]
pub(crate) struct RunState {
    inner: Arc<RwLock<HashMap<Uuid, RunEntry>>>,
}

/// Private mutable state for a single run.
pub(crate) struct RunEntry {
    pub actor_id: Uuid,
    pub status: RunStatus,
    pub created_at: Timestamp,
    pub started_at: Option<Timestamp>,
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
            started_at: self.started_at,
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
            started_at: self.started_at,
            completed_at: self.completed_at,
            node_count: self.nodes.len(),
        }
    }
}

impl RunState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Insert a new run entry.
    pub async fn insert(&self, run_id: Uuid, entry: RunEntry) {
        self.inner.write().await.insert(run_id, entry);
    }

    /// Update a single node's snapshot within a run.
    ///
    /// Returns `true` if the node was found and updated, `false` otherwise.
    pub async fn update_node(
        &self,
        run_id: Uuid,
        node_id: Uuid,
        status: NodeStatus,
        items_processed: u64,
        error: Option<String>,
    ) -> bool {
        let mut guard = self.inner.write().await;
        let Some(entry) = guard.get_mut(&run_id) else {
            tracing::warn!(%run_id, %node_id, "update_node: run not found");
            return false;
        };
        let Some(node) = entry.nodes.get_mut(&node_id) else {
            tracing::warn!(%run_id, %node_id, "update_node: node not found in run");
            return false;
        };
        node.status = status;
        node.items_processed = items_processed;
        node.error = error;
        true
    }

    /// Record the moment a run begins executing.
    pub async fn set_started_at(&self, run_id: Uuid) {
        if let Some(entry) = self.inner.write().await.get_mut(&run_id) {
            entry.started_at = Some(Timestamp::now());
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
        let mut guard = self.inner.write().await;
        if let Some(entry) = guard.get_mut(&run_id) {
            entry.status = status;
            entry.completed_at = Some(Timestamp::now());
            entry.entities_detected = entities_detected;
            entry.redactions_applied = redactions_applied;

            for node in entry.nodes.values_mut() {
                match node.status {
                    NodeStatus::Pending => {
                        node.status = NodeStatus::Failed;
                        node.error = Some("run completed before node was scheduled".to_string());
                    }
                    NodeStatus::Running => {
                        node.status = NodeStatus::Failed;
                        node.error = Some("run completed while node was still running".to_string());
                    }
                    _ => {}
                }
            }
        }
    }

    /// Mark a run as failed without entity/redaction counts.
    pub async fn fail(&self, run_id: Uuid) {
        self.finalize(run_id, RunStatus::Failed, 0, 0).await;
    }

    /// Get a full snapshot of a single run.
    ///
    /// Returns `None` if the run does not exist or belongs to a different actor.
    pub async fn get_run(&self, actor_id: Uuid, id: Uuid) -> Option<RunSnapshot> {
        self.inner
            .read()
            .await
            .get(&id)
            .filter(|entry| entry.actor_id == actor_id)
            .map(|entry| entry.to_snapshot(id))
    }

    /// List runs matching the given filter, scoped to the given actor.
    pub async fn list_runs(&self, actor_id: Uuid, filter: &RunFilter) -> Vec<RunSummary> {
        self.inner
            .read()
            .await
            .iter()
            .filter(|(_, entry)| {
                entry.actor_id == actor_id && filter.status.is_none_or(|s| entry.status == s)
            })
            .map(|(&id, entry)| entry.to_summary(id))
            .collect()
    }

    /// Request cancellation of an in-progress run.
    ///
    /// Returns a `NotFound` error if the run does not exist or belongs
    /// to a different actor.
    pub async fn cancel_run(&self, actor_id: Uuid, id: Uuid) -> Result<(), nvisy_core::Error> {
        let mut guard = self.inner.write().await;
        let entry = guard
                        .get_mut(&id)
            .filter(|e| e.actor_id == actor_id)
            .ok_or_else(|| {
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
    /// Returns `Err` if the run does not exist, belongs to a different
    /// actor, or is still active.
    pub async fn delete_run(&self, actor_id: Uuid, id: Uuid) -> Result<(), nvisy_core::Error> {
        let mut guard = self.inner.write().await;
        let entry = guard
                        .get(&id)
            .filter(|e| e.actor_id == actor_id)
            .ok_or_else(|| {
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

        guard.remove(&id);
        Ok(())
    }

    /// Remove all finished runs belonging to the given actor.
    ///
    /// Active runs (pending or running) are preserved. Returns the
    /// number of removed entries.
    pub async fn delete_all_runs(&self, actor_id: Uuid) -> usize {
        let mut guard = self.inner.write().await;
        let before = guard.len();
        guard.retain(|_, entry| {
            entry.actor_id != actor_id
                || matches!(entry.status, RunStatus::Pending | RunStatus::Running)
        });
        before - guard.len()
    }

    /// Collect a point-in-time analytics snapshot.
    pub async fn snapshot(&self) -> AnalyticsSnapshot {
        let guard = self.inner.read().await;
        let mut active = 0u64;
        let mut succeeded = 0u64;
        let mut failed = 0u64;
        let mut cancelled = 0u64;
        let mut total_entities = 0u64;
        let mut total_redactions = 0u64;
        let mut actors = std::collections::HashSet::new();
        let mut durations_ms: Vec<u64> = Vec::new();

        for entry in guard.values() {
            match entry.status {
                RunStatus::Pending | RunStatus::Running => active += 1,
                RunStatus::Succeeded => succeeded += 1,
                RunStatus::Failed | RunStatus::PartialFailure => failed += 1,
                RunStatus::Cancelled => cancelled += 1,
            }
            actors.insert(entry.actor_id);
            total_entities += entry.entities_detected;
            total_redactions += entry.redactions_applied;

            if let Some(completed_at) = entry.completed_at {
                let span = completed_at.since(entry.created_at);
                if let Ok(ms) = span.and_then(|s| s.total(jiff::Unit::Millisecond)) {
                    durations_ms.push(ms as u64);
                }
            }
        }

        let (min_run_duration_ms, max_run_duration_ms, avg_run_duration_ms) =
            if durations_ms.is_empty() {
                (None, None, None)
            } else {
                let min = *durations_ms.iter().min().unwrap();
                let max = *durations_ms.iter().max().unwrap();
                let sum: u64 = durations_ms.iter().sum();
                let avg = sum as f64 / durations_ms.len() as f64;
                (Some(min), Some(max), Some(avg))
            };

        AnalyticsSnapshot {
            timestamp: Timestamp::now(),
            total_runs: guard.len() as u64,
            active_runs: active,
            succeeded_runs: succeeded,
            failed_runs: failed,
            cancelled_runs: cancelled,
            total_entities_detected: total_entities,
            total_redactions_applied: total_redactions,
            distinct_actors: actors.len() as u64,
            min_run_duration_ms,
            max_run_duration_ms,
            avg_run_duration_ms,
        }
    }
}

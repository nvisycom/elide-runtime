//! Volatile, in-memory run state storage.
//!
//! [`RunState`] wraps an `Arc<RwLock<HashMap<Uuid, RunRecord>>>` providing
//! concurrent read/write access to run records. It is cheaply clonable
//! (single `Arc` bump) and shared between the [`Engine`] and the
//! [`orchestrator`].
//!
//! [`Engine`]: super::super::Engine
//! [`orchestrator`]: super::super::orchestrator
//!
//! All queries are scoped by `actor_id` — an actor can only see and
//! mutate their own runs. Finalization forces any still-pending or
//! still-running nodes into `Failed` status to ensure every node reaches
//! a terminal state.
//!
//! All data is lost on process restart.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use jiff::Timestamp;
use nvisy_core::{Error, ErrorKind};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{
    AnalyticsSnapshot, NodeSnapshot, NodeStatus, RunEntry, RunFilter, RunOutcome, RunSnapshot,
    RunStatus,
};

/// In-memory run state backed by `Arc<RwLock<HashMap>>`.
///
/// Cheaply clonable (Arc bump). All clones share the same underlying
/// data. The orchestrator and engine both hold clones to read/write
/// run progress concurrently.
#[derive(Clone)]
pub(crate) struct RunState {
    inner: Arc<RwLock<HashMap<Uuid, RunRecord>>>,
}

/// Mutable state for a single pipeline run.
///
/// Tracks lifecycle timestamps, per-node snapshots, aggregate counters,
/// and a [`CancellationToken`] for cooperative cancellation.
pub(crate) struct RunRecord {
    /// Identity of the actor who initiated this run.
    pub actor_id: Uuid,
    /// Current lifecycle status.
    pub status: RunStatus,
    /// When the run was first created (before compilation).
    pub created_at: Timestamp,
    /// When DAG execution actually began (after compilation + context loading).
    pub started_at: Option<Timestamp>,
    /// When the run reached a terminal state.
    pub completed_at: Option<Timestamp>,
    /// Per-node progress snapshots, keyed by node ID.
    pub nodes: HashMap<Uuid, NodeSnapshot>,
    /// Token shared with all node tasks for cooperative cancellation.
    pub cancel: CancellationToken,
    /// Running total of entities detected across all nodes.
    pub entities_detected: u64,
    /// Running total of redactions applied across all nodes.
    pub redactions_applied: u64,
    /// Error message for failed runs.
    pub error: Option<String>,
}

impl RunRecord {
    /// Project this entry into a full [`RunSnapshot`] for API responses.
    ///
    /// Terminal outcomes (`Succeeded`, `PartialFailure`) are built with
    /// empty audit vecs — [`Engine::get_run`] populates them from the
    /// registry.
    fn to_snapshot(&self, id: Uuid) -> RunSnapshot {
        let outcome = match self.status {
            RunStatus::Pending => RunOutcome::Pending,
            RunStatus::Running => RunOutcome::Running,
            RunStatus::Succeeded => RunOutcome::Succeeded {
                completed_at: self.completed_at.unwrap_or_else(Timestamp::now),
                audits: vec![],
                entities_detected: self.entities_detected,
                redactions_applied: self.redactions_applied,
            },
            RunStatus::PartialFailure => RunOutcome::PartialFailure {
                completed_at: self.completed_at.unwrap_or_else(Timestamp::now),
                audits: vec![],
                entities_detected: self.entities_detected,
                redactions_applied: self.redactions_applied,
            },
            RunStatus::Failed => RunOutcome::Failed {
                completed_at: self.completed_at.unwrap_or_else(Timestamp::now),
                error: self.error.clone(),
            },
            RunStatus::Cancelled => RunOutcome::Cancelled {
                completed_at: self.completed_at.unwrap_or_else(Timestamp::now),
            },
        };

        RunSnapshot {
            id,
            actor_id: self.actor_id,
            created_at: self.created_at,
            started_at: self.started_at,
            nodes: {
                let mut nodes: Vec<_> = self.nodes.values().cloned().collect();
                nodes.sort_by_key(|n| n.node_id);
                nodes
            },
            outcome,
        }
    }

    /// Project this entry into a lightweight [`RunEntry`] for listing.
    fn to_summary(&self, id: Uuid) -> RunEntry {
        RunEntry {
            id,
            actor_id: self.actor_id,
            status: self.status,
            created_at: self.created_at,
            started_at: self.started_at,
            completed_at: self.completed_at,
            node_count: self.nodes.len(),
            entities_detected: self.entities_detected,
            redactions_applied: self.redactions_applied,
        }
    }
}

impl RunState {
    /// Create an empty run store.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Insert a new run entry, keyed by its unique run ID.
    pub async fn insert(&self, run_id: Uuid, entry: RunRecord) {
        self.inner.write().await.insert(run_id, entry);
    }

    /// Record the moment a run begins executing.
    pub async fn set_started_at(&self, run_id: Uuid) {
        if let Some(entry) = self.inner.write().await.get_mut(&run_id) {
            entry.started_at = Some(Timestamp::now());
        }
    }

    /// Transition a run to its final status and record aggregate counters.
    ///
    /// If the run was already cancelled (via [`cancel_run`]),
    /// the `Cancelled` status is preserved rather than being overwritten by
    /// the orchestrator's computed status.
    ///
    /// Any nodes still in `Pending` or `Running` state are forced to
    /// `Failed` with an explanatory error message, ensuring all nodes
    /// reach a terminal status.
    ///
    /// [`cancel_run`]: Self::cancel_run
    pub async fn finalize(
        &self,
        run_id: Uuid,
        status: RunStatus,
        entities_detected: u64,
        redactions_applied: u64,
    ) {
        let mut guard = self.inner.write().await;
        if let Some(entry) = guard.get_mut(&run_id) {
            if entry.status != RunStatus::Cancelled {
                entry.status = status;
            }
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

    /// Shorthand to mark a run as [`RunStatus::Failed`] with zero counters.
    pub async fn fail(&self, run_id: Uuid, error: impl Into<String>) {
        let error = error.into();
        let mut guard = self.inner.write().await;
        if let Some(entry) = guard.get_mut(&run_id) {
            entry.error = Some(error);
        }
        drop(guard);
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
    pub async fn list_runs(&self, actor_id: Uuid, filter: &RunFilter) -> Vec<RunEntry> {
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
    pub async fn cancel_run(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        let mut guard = self.inner.write().await;
        let entry = guard
            .get_mut(&id)
            .filter(|e| e.actor_id == actor_id)
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "run not found"))?;

        match entry.status {
            RunStatus::Pending | RunStatus::Running => {
                entry.cancel.cancel();
                entry.status = RunStatus::Cancelled;
                entry.completed_at = Some(Timestamp::now());
                Ok(())
            }
            _ => Err(
                Error::new(ErrorKind::Validation, "run has already finished").with_component("run"),
            ),
        }
    }

    /// Remove a single finished run from the store.
    ///
    /// Returns `Err` if the run does not exist, belongs to a different
    /// actor, or is still active (pending/running).
    pub async fn delete_run(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        let mut guard = self.inner.write().await;
        let entry = guard
            .get(&id)
            .filter(|e| e.actor_id == actor_id)
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "run not found"))?;

        match entry.status {
            RunStatus::Pending | RunStatus::Running => {
                return Err(
                    Error::new(ErrorKind::Validation, "cannot delete an active run")
                        .with_component("run"),
                );
            }
            _ => {}
        }

        guard.remove(&id);
        Ok(())
    }

    /// Remove all finished runs belonging to the given actor.
    ///
    /// Active runs (pending or running) are preserved. Returns the
    /// IDs of removed entries.
    pub async fn delete_all_runs(&self, actor_id: Uuid) -> Vec<Uuid> {
        let mut guard = self.inner.write().await;
        let to_remove: Vec<Uuid> = guard
            .iter()
            .filter(|(_, entry)| {
                entry.actor_id == actor_id
                    && !matches!(entry.status, RunStatus::Pending | RunStatus::Running)
            })
            .map(|(&id, _)| id)
            .collect();
        for id in &to_remove {
            guard.remove(id);
        }
        to_remove
    }

    /// Compute a point-in-time [`AnalyticsSnapshot`] from all tracked runs.
    pub async fn snapshot(&self) -> AnalyticsSnapshot {
        let guard = self.inner.read().await;
        let mut current = 0u64;
        let mut succeeded = 0u64;
        let mut failed = 0u64;
        let mut cancelled = 0u64;
        let mut actors = HashSet::new();
        let mut durations_ms: Vec<u64> = Vec::new();

        for entry in guard.values() {
            match entry.status {
                RunStatus::Pending | RunStatus::Running => current += 1,
                RunStatus::Succeeded => succeeded += 1,
                RunStatus::Failed | RunStatus::PartialFailure => failed += 1,
                RunStatus::Cancelled => cancelled += 1,
            }
            actors.insert(entry.actor_id);

            if let Some(completed_at) = entry.completed_at {
                let span = completed_at.since(entry.created_at);
                if let Ok(ms) = span.and_then(|s| s.total(jiff::Unit::Millisecond)) {
                    durations_ms.push(ms as u64);
                }
            }
        }

        let (max_run_duration_ms, avg_run_duration_ms) = if durations_ms.is_empty() {
            (None, None)
        } else {
            let max = *durations_ms.iter().max().unwrap();
            let sum: u64 = durations_ms.iter().sum();
            let avg = sum as f64 / durations_ms.len() as f64;
            (Some(max), Some(avg))
        };

        AnalyticsSnapshot {
            timestamp: Timestamp::now(),
            current_runs: current,
            succeeded_runs: succeeded,
            failed_runs: failed,
            cancelled_runs: cancelled,
            distinct_actors: actors.len() as u64,
            max_run_duration_ms,
            avg_run_duration_ms,
        }
    }
}

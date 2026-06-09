//! Volatile in-memory state for active detection passes.
//!
//! Analog of the deleted `RunState`. Holds per-pass timestamps,
//! status, cancellation token, and a running audit accumulator
//! that's frozen into a [`DetectionResult`] once the pass reaches
//! a terminal state.

use std::collections::HashMap;
use std::sync::Arc;

use jiff::Timestamp;
use nvisy_core::Error;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::result::{DetectionEntry, DetectionFilter, DetectionResult, DetectionSnapshot};
use super::status::DetectionStatus;
use crate::phases::ingestion::ImportFile;
use crate::provenance::AnyAudit;

const TARGET: &str = "nvisy_engine::detection::state";

/// In-memory detection-pass tracker backed by `Arc<RwLock<HashMap>>`.
///
/// Cheaply clonable (Arc bump). The engine and the detection
/// orchestrator both hold clones for concurrent read/write.
#[derive(Clone, Default)]
pub(crate) struct DetectionState {
    inner: Arc<RwLock<HashMap<Uuid, DetectionRecord>>>,
}

/// Mutable per-pass state.
pub(crate) struct DetectionRecord {
    pub actor_id: Uuid,
    pub policies: Vec<Uuid>,
    pub imports: Vec<ImportFile>,
    pub status: DetectionStatus,
    pub created_at: Timestamp,
    pub started_at: Option<Timestamp>,
    pub completed_at: Option<Timestamp>,
    pub cancel: CancellationToken,
    pub audits: Vec<AnyAudit>,
    pub entities_detected: u64,
    pub error: Option<String>,
}

impl DetectionRecord {
    fn to_snapshot(&self, id: Uuid) -> DetectionSnapshot {
        let result = match self.status {
            DetectionStatus::Succeeded | DetectionStatus::PartialFailure => Some(DetectionResult {
                id,
                actor_id: self.actor_id,
                policies: self.policies.clone(),
                imports: self.imports.clone(),
                audits: self.audits.clone(),
                entities_detected: self.entities_detected,
            }),
            _ => None,
        };
        DetectionSnapshot {
            id,
            actor_id: self.actor_id,
            status: self.status,
            created_at: self.created_at,
            started_at: self.started_at,
            completed_at: self.completed_at,
            result,
            error: self.error.clone(),
        }
    }

    fn to_entry(&self, id: Uuid) -> DetectionEntry {
        DetectionEntry {
            id,
            actor_id: self.actor_id,
            status: self.status,
            created_at: self.created_at,
            started_at: self.started_at,
            completed_at: self.completed_at,
            import_count: self.imports.len(),
            entities_detected: self.entities_detected,
        }
    }
}

impl DetectionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert(&self, id: Uuid, record: DetectionRecord) {
        self.inner.write().await.insert(id, record);
    }

    /// Look up a snapshot scoped by actor.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::NotFound`] when the detection does not exist
    /// or belongs to a different actor (the two cases share an
    /// error kind to avoid leaking existence to unauthorised
    /// callers).
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    pub async fn snapshot(&self, actor_id: Uuid, id: Uuid) -> Result<DetectionSnapshot, Error> {
        let guard = self.inner.read().await;
        let Some(record) = guard.get(&id) else {
            return Err(Error::not_found(
                format!("detection {id} not found"),
                TARGET,
            ));
        };
        if record.actor_id != actor_id {
            return Err(Error::not_found(
                format!("detection {id} not found"),
                TARGET,
            ));
        }
        Ok(record.to_snapshot(id))
    }

    /// Resolve a [`DetectionResult`] for redaction.
    ///
    /// # Errors
    ///
    /// - [`ErrorKind::NotFound`] when the detection does not
    ///   exist or belongs to a different actor. The two cases
    ///   share an error kind on purpose — actor-scoping leaks
    ///   information if "exists for other actor" is
    ///   distinguishable from "does not exist."
    /// - [`ErrorKind::Conflict`] when the detection exists for
    ///   this actor but is not in a terminal succeeded /
    ///   partial-failure state. Includes the current status in
    ///   the message so the caller can poll.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    /// [`ErrorKind::Conflict`]: nvisy_core::ErrorKind::Conflict
    pub async fn result(&self, actor_id: Uuid, id: Uuid) -> Result<DetectionResult, Error> {
        let guard = self.inner.read().await;
        let Some(record) = guard.get(&id) else {
            return Err(Error::not_found(
                format!("detection {id} not found"),
                TARGET,
            ));
        };
        if record.actor_id != actor_id {
            return Err(Error::not_found(
                format!("detection {id} not found"),
                TARGET,
            ));
        }
        if !matches!(
            record.status,
            DetectionStatus::Succeeded | DetectionStatus::PartialFailure
        ) {
            return Err(Error::conflict(
                format!(
                    "detection {id} not ready for redaction (status: {:?})",
                    record.status
                ),
                TARGET,
            ));
        }
        Ok(DetectionResult {
            id,
            actor_id: record.actor_id,
            policies: record.policies.clone(),
            imports: record.imports.clone(),
            audits: record.audits.clone(),
            entities_detected: record.entities_detected,
        })
    }

    pub async fn list(&self, actor_id: Uuid, filter: DetectionFilter) -> Vec<DetectionEntry> {
        let guard = self.inner.read().await;
        let mut out: Vec<DetectionEntry> = guard
            .iter()
            .filter(|(_, r)| r.actor_id == actor_id)
            .filter(|(_, r)| filter.status.is_none_or(|s| r.status == s))
            .map(|(id, r)| r.to_entry(*id))
            .collect();
        out.sort_by_key(|e| std::cmp::Reverse(e.created_at));
        out
    }

    pub async fn set_started_at(&self, id: Uuid) {
        let mut guard = self.inner.write().await;
        if let Some(record) = guard.get_mut(&id)
            && record.started_at.is_none()
        {
            record.started_at = Some(Timestamp::now());
            record.status = DetectionStatus::Running;
        }
    }

    pub async fn fail(&self, id: Uuid, error: impl Into<String>) {
        let mut guard = self.inner.write().await;
        if let Some(record) = guard.get_mut(&id) {
            record.status = DetectionStatus::Failed;
            record.completed_at = Some(Timestamp::now());
            record.error = Some(error.into());
        }
    }

    pub async fn finalize(
        &self,
        id: Uuid,
        status: DetectionStatus,
        audits: Vec<AnyAudit>,
        entities_detected: u64,
    ) {
        let mut guard = self.inner.write().await;
        if let Some(record) = guard.get_mut(&id) {
            record.status = status;
            record.completed_at = Some(Timestamp::now());
            record.audits = audits;
            record.entities_detected = entities_detected;
        }
    }

    /// Cancel an in-progress detection.
    ///
    /// # Errors
    ///
    /// - [`ErrorKind::NotFound`] when the detection does not
    ///   exist or belongs to a different actor.
    /// - [`ErrorKind::Conflict`] when the detection is already
    ///   in a terminal state.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    /// [`ErrorKind::Conflict`]: nvisy_core::ErrorKind::Conflict
    pub async fn cancel(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        let mut guard = self.inner.write().await;
        let Some(record) = guard.get_mut(&id) else {
            return Err(Error::not_found(
                format!("detection {id} not found"),
                TARGET,
            ));
        };
        if record.actor_id != actor_id {
            return Err(Error::not_found(
                format!("detection {id} not found"),
                TARGET,
            ));
        }
        if record.status.is_terminal() {
            return Err(Error::conflict(
                format!(
                    "detection {id} already in terminal state {:?}",
                    record.status
                ),
                TARGET,
            ));
        }
        record.cancel.cancel();
        record.status = DetectionStatus::Cancelled;
        record.completed_at = Some(Timestamp::now());
        Ok(())
    }

    /// Delete a finished detection.
    ///
    /// # Errors
    ///
    /// - [`ErrorKind::NotFound`] when the detection does not
    ///   exist or belongs to a different actor.
    /// - [`ErrorKind::Conflict`] when the detection is still
    ///   active.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    /// [`ErrorKind::Conflict`]: nvisy_core::ErrorKind::Conflict
    pub async fn delete(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        let mut guard = self.inner.write().await;
        let Some(record) = guard.get(&id) else {
            return Err(Error::not_found(
                format!("detection {id} not found"),
                TARGET,
            ));
        };
        if record.actor_id != actor_id {
            return Err(Error::not_found(
                format!("detection {id} not found"),
                TARGET,
            ));
        }
        if !record.status.is_terminal() {
            return Err(Error::conflict(
                format!("detection {id} cannot be deleted while {:?}", record.status),
                TARGET,
            ));
        }
        guard.remove(&id);
        Ok(())
    }

    /// Delete every terminal-status detection for `actor_id`.
    /// Returns the ids removed so the caller can cascade the
    /// delete to other stores (registry persistence).
    pub async fn delete_all(&self, actor_id: Uuid) -> Vec<Uuid> {
        let mut guard = self.inner.write().await;
        let to_delete: Vec<Uuid> = guard
            .iter()
            .filter(|(_, r)| r.actor_id == actor_id && r.status.is_terminal())
            .map(|(id, _)| *id)
            .collect();
        for id in &to_delete {
            guard.remove(id);
        }
        to_delete
    }
}

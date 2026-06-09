//! Volatile in-memory state for active redaction passes.

use std::collections::HashMap;
use std::sync::Arc;

use jiff::Timestamp;
use nvisy_core::Error;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::result::{RedactionEntry, RedactionFilter, RedactionResult, RedactionSnapshot};
use super::status::RedactionStatus;
use crate::document::provenance::AnyAudit;

const TARGET: &str = "nvisy_engine::redaction::state";

#[derive(Clone, Default)]
pub(crate) struct RedactionState {
    inner: Arc<RwLock<HashMap<Uuid, RedactionRecord>>>,
}

pub(crate) struct RedactionRecord {
    pub detection_id: Uuid,
    pub actor_id: Uuid,
    pub status: RedactionStatus,
    pub created_at: Timestamp,
    pub started_at: Option<Timestamp>,
    pub completed_at: Option<Timestamp>,
    pub cancel: CancellationToken,
    pub audits: Vec<AnyAudit>,
    pub redactions_applied: u64,
    pub error: Option<String>,
}

impl RedactionRecord {
    fn to_snapshot(&self, id: Uuid) -> RedactionSnapshot {
        let result = match self.status {
            RedactionStatus::Succeeded | RedactionStatus::PartialFailure => Some(RedactionResult {
                id,
                detection_id: self.detection_id,
                actor_id: self.actor_id,
                audits: self.audits.clone(),
                redactions_applied: self.redactions_applied,
            }),
            _ => None,
        };
        RedactionSnapshot {
            id,
            detection_id: self.detection_id,
            actor_id: self.actor_id,
            status: self.status,
            created_at: self.created_at,
            started_at: self.started_at,
            completed_at: self.completed_at,
            result,
            error: self.error.clone(),
        }
    }

    fn to_entry(&self, id: Uuid) -> RedactionEntry {
        RedactionEntry {
            id,
            detection_id: self.detection_id,
            actor_id: self.actor_id,
            status: self.status,
            created_at: self.created_at,
            started_at: self.started_at,
            completed_at: self.completed_at,
            redactions_applied: self.redactions_applied,
        }
    }
}

impl RedactionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert(&self, id: Uuid, record: RedactionRecord) {
        self.inner.write().await.insert(id, record);
    }

    /// Look up a snapshot scoped by actor.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::NotFound`] when the redaction does not exist
    /// or belongs to a different actor.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    pub async fn snapshot(&self, actor_id: Uuid, id: Uuid) -> Result<RedactionSnapshot, Error> {
        let guard = self.inner.read().await;
        let Some(record) = guard.get(&id) else {
            return Err(Error::not_found(
                format!("redaction {id} not found"),
                TARGET,
            ));
        };
        if record.actor_id != actor_id {
            return Err(Error::not_found(
                format!("redaction {id} not found"),
                TARGET,
            ));
        }
        Ok(record.to_snapshot(id))
    }

    pub async fn list(&self, actor_id: Uuid, filter: RedactionFilter) -> Vec<RedactionEntry> {
        let guard = self.inner.read().await;
        let mut out: Vec<RedactionEntry> = guard
            .iter()
            .filter(|(_, r)| r.actor_id == actor_id)
            .filter(|(_, r)| filter.status.is_none_or(|s| r.status == s))
            .filter(|(_, r)| filter.detection_id.is_none_or(|d| r.detection_id == d))
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
            record.status = RedactionStatus::Running;
        }
    }

    pub async fn fail(&self, id: Uuid, error: impl Into<String>) {
        let mut guard = self.inner.write().await;
        if let Some(record) = guard.get_mut(&id) {
            record.status = RedactionStatus::Failed;
            record.completed_at = Some(Timestamp::now());
            record.error = Some(error.into());
        }
    }

    pub async fn finalize(
        &self,
        id: Uuid,
        status: RedactionStatus,
        audits: Vec<AnyAudit>,
        redactions_applied: u64,
    ) {
        let mut guard = self.inner.write().await;
        if let Some(record) = guard.get_mut(&id) {
            record.status = status;
            record.completed_at = Some(Timestamp::now());
            record.audits = audits;
            record.redactions_applied = redactions_applied;
        }
    }

    /// Cancel an in-progress redaction.
    ///
    /// # Errors
    ///
    /// - [`ErrorKind::NotFound`] when the redaction does not
    ///   exist or belongs to a different actor.
    /// - [`ErrorKind::Conflict`] when the redaction is already
    ///   in a terminal state.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    /// [`ErrorKind::Conflict`]: nvisy_core::ErrorKind::Conflict
    pub async fn cancel(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        let mut guard = self.inner.write().await;
        let Some(record) = guard.get_mut(&id) else {
            return Err(Error::not_found(
                format!("redaction {id} not found"),
                TARGET,
            ));
        };
        if record.actor_id != actor_id {
            return Err(Error::not_found(
                format!("redaction {id} not found"),
                TARGET,
            ));
        }
        if record.status.is_terminal() {
            return Err(Error::conflict(
                format!(
                    "redaction {id} already in terminal state {:?}",
                    record.status
                ),
                TARGET,
            ));
        }
        record.cancel.cancel();
        record.status = RedactionStatus::Cancelled;
        record.completed_at = Some(Timestamp::now());
        Ok(())
    }

    /// Delete a finished redaction.
    ///
    /// # Errors
    ///
    /// - [`ErrorKind::NotFound`] when the redaction does not
    ///   exist or belongs to a different actor.
    /// - [`ErrorKind::Conflict`] when the redaction is still
    ///   active.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    /// [`ErrorKind::Conflict`]: nvisy_core::ErrorKind::Conflict
    pub async fn delete(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        let mut guard = self.inner.write().await;
        let Some(record) = guard.get(&id) else {
            return Err(Error::not_found(
                format!("redaction {id} not found"),
                TARGET,
            ));
        };
        if record.actor_id != actor_id {
            return Err(Error::not_found(
                format!("redaction {id} not found"),
                TARGET,
            ));
        }
        if !record.status.is_terminal() {
            return Err(Error::conflict(
                format!("redaction {id} cannot be deleted while {:?}", record.status),
                TARGET,
            ));
        }
        guard.remove(&id);
        Ok(())
    }

    /// Delete every terminal-status redaction for `actor_id`.
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

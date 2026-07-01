//! Retention-schedule storage.
//!
//! Surfaced as the crate-private [`RetentionRegistry`] extension
//! trait on [`RegistryHandle`] so the run lifecycle stamps + the
//! sweeper read through the same shape every other run-side store
//! uses (`registry.put_retention(...)`, `list_due(...)`, etc.).
//! Trait is `pub(crate)`: retention rows are the engine's
//! invariant, never written directly from outside the engine —
//! [`Engine::start_run`] pins them at run start,
//! [`Engine::apply_run`] pins the output rows after each redacted
//! file lands, and the sweeper deletes them as artifacts expire.
//!
//! ## Keyspace
//!
//! One keyspace, `retention_schedule`, keyed by
//! [`RetentionKey`] = `(actor_id, file_id, scope)` (33 bytes).
//! Values are JSON-encoded [`RetentionRecord`]. Only `Zero` and
//! `Duration` retentions get rows; `Indefinite` is the absence of
//! a row, so the sweeper has nothing to scan for never-deleted
//! artifacts.
//!
//! Deadlines are stored on the value, not in the key — the
//! keyspace is `(actor, file, scope)`-ordered, not
//! deadline-ordered. [`RetentionRegistry::list_due_retention`]
//! does a full scan; a secondary index keyed by deadline is a
//! future optimisation.
//!
//! [`Engine::start_run`]: crate::Engine::start_run
//! [`Engine::apply_run`]: crate::Engine::apply_run
//! [`RetentionKey`]: crate::registry::RetentionKey
//! [`RegistryHandle`]: crate::registry::RegistryHandle

use std::collections::HashMap;
use std::error::Error as StdError;

use jiff::Timestamp;
use nvisy_core::policy::{Retention, RetentionScope};
use nvisy_core::{Error, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::registry::{RegistryHandle, RetentionKey, blocking};

const COMPONENT: &str = "retention::schedule";

/// One retention schedule row: which scope governs the file,
/// when it expires, and which run pinned the rule. Self-describing
/// (`scope` is also on the key) so the sweeper can act on a row
/// without re-decoding the key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionRecord {
    /// The scope this row applies to. Redundant with the key's
    /// trailing byte, kept on the value for self-describing rows.
    pub scope: RetentionScope,
    /// UTC instant at which the artifact is eligible for
    /// deletion. The sweeper compares `deadline <= now` to decide
    /// whether the row is due.
    pub deadline: Timestamp,
    /// Run that pinned this retention. Lets audits trace a
    /// deletion back to the policy that drove it.
    pub source_run_id: Uuid,
}

impl RetentionRecord {
    /// Materialise a record from a resolved [`Retention`].
    ///
    /// Returns `None` for [`Retention::Indefinite`]: the absence
    /// of a row IS the encoding for "keep indefinitely", so the
    /// sweeper has nothing to scan for never-deleted artifacts.
    /// `Zero` produces a record with `deadline = pinned_at` so
    /// the next sweeper tick collects it.
    pub fn from_retention(
        scope: RetentionScope,
        retention: Retention,
        source_run_id: Uuid,
        pinned_at: Timestamp,
    ) -> Option<Self> {
        let deadline = match retention {
            Retention::ZeroRetention => pinned_at,
            Retention::Duration { days } => {
                // `Timestamp + Span` is jiff's saturating-on-overflow
                // arithmetic at this resolution; days fits in i64
                // easily for any human-meaningful retention window.
                let span = jiff::SignedDuration::from_secs(
                    days.saturating_mul(24 * 60 * 60).min(i64::MAX as u64) as i64,
                );
                pinned_at.checked_add(span).ok()?
            }
            Retention::Indefinite => return None,
            // `Retention` is `#[non_exhaustive]`. A future variant
            // reaching this match in an older binary should be
            // treated as indefinite (no row) rather than crashing
            // — the engine doesn't know how to encode it.
            _ => return None,
        };
        Some(RetentionRecord {
            scope,
            deadline,
            source_run_id,
        })
    }

    /// Whether this row is past its deadline as of `now`.
    pub fn is_due(&self, now: Timestamp) -> bool {
        self.deadline <= now
    }
}

/// Crate-private extension trait adding retention-schedule
/// storage to [`RegistryHandle`].
pub(crate) trait RetentionRegistry {
    /// Write the retention row at `(actor_id, file_id, scope)`.
    /// Overwrites any existing row for the same triple — when a
    /// new run pins stricter retention for an artifact already
    /// scheduled, the new row replaces the old.
    fn put_retention(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
        record: &RetentionRecord,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Pin one scope's row on `(actor_id, file_id)` from a
    /// [`resolve_retention`] map. No-op when:
    /// - the map has no entry for `scope` (no policy governs it),
    /// - or the resolved retention is `Indefinite` (absence of a
    ///   row IS the encoding).
    ///
    /// Otherwise builds a [`RetentionRecord`] with `pinned_at =
    /// now` and writes it. Consolidates the pattern used by
    /// [`Engine::start_run`] (OriginalContent per input file)
    /// and the per-doc apply path (RedactedOutput per output
    /// file).
    ///
    /// [`resolve_retention`]: nvisy_core::policy::resolve_retention
    /// [`Engine::start_run`]: crate::Engine::start_run
    fn pin_retention(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
        scope: RetentionScope,
        resolved: &HashMap<RetentionScope, Retention>,
        source_run_id: Uuid,
        now: Timestamp,
    ) -> impl Future<Output = Result<()>> + Send
    where
        Self: Sync,
    {
        let record = resolved
            .get(&scope)
            .and_then(|r| RetentionRecord::from_retention(scope, *r, source_run_id, now));
        async move {
            match record {
                Some(record) => self.put_retention(actor_id, file_id, &record).await,
                None => Ok(()),
            }
        }
    }

    /// List every retention row for `(actor_id, file_id)`. Used
    /// by callers who want to know all retention rules governing
    /// a single artifact (e.g. policy diff diagnostics, or the
    /// sweeper checking whether to defer deletion).
    fn list_retention_for_file(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
    ) -> impl Future<Output = Result<Vec<RetentionRecord>>> + Send;

    /// Fetch the retention row for `(actor_id, file_id, scope)`,
    /// or `None` when no row exists (the absence encodes
    /// "indefinite" or "not yet scheduled").
    fn find_retention(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
        scope: RetentionScope,
    ) -> impl Future<Output = Result<Option<RetentionRecord>>> + Send
    where
        Self: Sync,
    {
        async move {
            Ok(self
                .list_retention_for_file(actor_id, file_id)
                .await?
                .into_iter()
                .find(|r| r.scope == scope))
        }
    }

    /// Delete the retention row at `(actor_id, file_id, scope)`.
    /// Idempotent — removing a row that already doesn't exist
    /// succeeds. The sweeper calls this after it has confirmed
    /// the artifact deletion.
    fn delete_retention(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
        scope: RetentionScope,
    ) -> impl Future<Output = Result<()>> + Send;

    /// List every retention row across every actor whose
    /// `deadline <= now`. The sweeper's read entry point.
    ///
    /// Returns the parsed key components alongside the record
    /// so the sweeper can act on `(actor, file)` without
    /// re-decoding bytes.
    fn list_due_retention(
        &self,
        now: Timestamp,
    ) -> impl Future<Output = Result<Vec<DueRetention>>> + Send;
}

/// A retention row the sweeper found due: the parsed key
/// components plus the record's payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DueRetention {
    pub actor_id: Uuid,
    pub file_id: Uuid,
    pub record: RetentionRecord,
}

impl RetentionRegistry for RegistryHandle {
    async fn put_retention(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
        record: &RetentionRecord,
    ) -> Result<()> {
        let key = RetentionKey::new(actor_id, file_id, record.scope);
        let value = serde_json::to_vec(record)?;
        let ks = self.retention_schedule().clone();
        blocking(move || {
            ks.insert(*key, value).map_err(fjall_err)?;
            Ok(())
        })
        .await
    }

    async fn list_retention_for_file(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
    ) -> Result<Vec<RetentionRecord>> {
        let ks = self.retention_schedule().clone();
        blocking(move || {
            let prefix = RetentionKey::file_prefix(actor_id, file_id);
            let mut out = Vec::new();
            for guard in ks.prefix(prefix) {
                let (_, value) = guard.into_inner().map_err(fjall_err)?;
                let record: RetentionRecord = serde_json::from_slice(&value)?;
                out.push(record);
            }
            Ok(out)
        })
        .await
    }

    async fn delete_retention(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
        scope: RetentionScope,
    ) -> Result<()> {
        let key = RetentionKey::new(actor_id, file_id, scope);
        let ks = self.retention_schedule().clone();
        blocking(move || {
            ks.remove(*key).map_err(fjall_err)?;
            Ok(())
        })
        .await
    }

    async fn list_due_retention(&self, now: Timestamp) -> Result<Vec<DueRetention>> {
        let ks = self.retention_schedule().clone();
        blocking(move || {
            let mut out = Vec::new();
            for guard in ks.iter() {
                let (key, value) = guard.into_inner().map_err(fjall_err)?;
                let record: RetentionRecord = serde_json::from_slice(&value)?;
                if !record.is_due(now) {
                    continue;
                }
                let actor_id = RetentionKey::actor_id_from_bytes(&key)
                    .ok_or_else(|| malformed_key_err(&key))?;
                let file_id = RetentionKey::file_id_from_bytes(&key)
                    .ok_or_else(|| malformed_key_err(&key))?;
                out.push(DueRetention {
                    actor_id,
                    file_id,
                    record,
                });
            }
            Ok(out)
        })
        .await
    }
}

fn fjall_err(err: impl StdError + Send + Sync + 'static) -> Error {
    Error::internal("fjall operation failed", COMPONENT).with_source(err)
}

fn malformed_key_err(key: &[u8]) -> Error {
    Error::internal(
        format!(
            "retention_schedule key has unexpected length {len}; expected 33",
            len = key.len(),
        ),
        COMPONENT,
    )
}

#[cfg(test)]
mod tests {
    use jiff::SignedDuration;
    use tempfile::TempDir;

    use super::*;

    fn registry() -> (RegistryHandle, TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let handle = RegistryHandle::open(dir.path()).expect("registry opens");
        (handle, dir)
    }

    fn now() -> Timestamp {
        Timestamp::from_second(1_700_000_000).expect("fixed timestamp")
    }

    #[test]
    fn zero_retention_deadline_equals_pinned_at() {
        let record = RetentionRecord::from_retention(
            RetentionScope::OriginalContent,
            Retention::ZeroRetention,
            Uuid::nil(),
            now(),
        )
        .expect("Zero retention produces a row");
        assert_eq!(record.deadline, now());
        assert_eq!(record.scope, RetentionScope::OriginalContent);
    }

    #[test]
    fn duration_retention_deadline_adds_days() {
        let record = RetentionRecord::from_retention(
            RetentionScope::RedactedOutput,
            Retention::Duration { days: 7 },
            Uuid::nil(),
            now(),
        )
        .expect("Duration retention produces a row");
        let expected = now()
            .checked_add(SignedDuration::from_secs(7 * 24 * 60 * 60))
            .expect("delta fits");
        assert_eq!(record.deadline, expected);
    }

    #[test]
    fn indefinite_retention_yields_no_row() {
        let record = RetentionRecord::from_retention(
            RetentionScope::AuditLogs,
            Retention::Indefinite,
            Uuid::nil(),
            now(),
        );
        assert!(record.is_none());
    }

    #[test]
    fn is_due_compares_against_now() {
        let record = RetentionRecord {
            scope: RetentionScope::OriginalContent,
            deadline: now(),
            source_run_id: Uuid::nil(),
        };
        let earlier = now().checked_sub(SignedDuration::from_secs(1)).unwrap();
        let later = now().checked_add(SignedDuration::from_secs(1)).unwrap();
        assert!(!record.is_due(earlier));
        assert!(record.is_due(now()));
        assert!(record.is_due(later));
    }

    #[tokio::test]
    async fn put_then_get_round_trips() {
        let (registry, _dir) = registry();
        let actor = Uuid::now_v7();
        let file = Uuid::now_v7();
        let run = Uuid::now_v7();
        let record = RetentionRecord {
            scope: RetentionScope::OriginalContent,
            deadline: now(),
            source_run_id: run,
        };
        registry
            .put_retention(actor, file, &record)
            .await
            .expect("put succeeds");
        let got = registry
            .find_retention(actor, file, RetentionScope::OriginalContent)
            .await
            .expect("find succeeds")
            .expect("row exists");
        assert_eq!(got, record);
    }

    #[tokio::test]
    async fn get_missing_returns_none() {
        let (registry, _dir) = registry();
        let got = registry
            .find_retention(
                Uuid::now_v7(),
                Uuid::now_v7(),
                RetentionScope::OriginalContent,
            )
            .await
            .expect("find succeeds");
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn put_overwrites_same_triple() {
        let (registry, _dir) = registry();
        let actor = Uuid::now_v7();
        let file = Uuid::now_v7();
        let earlier = now();
        let later = now().checked_add(SignedDuration::from_secs(3600)).unwrap();
        let first = RetentionRecord {
            scope: RetentionScope::OriginalContent,
            deadline: later,
            source_run_id: Uuid::now_v7(),
        };
        let stricter = RetentionRecord {
            scope: RetentionScope::OriginalContent,
            deadline: earlier,
            source_run_id: Uuid::now_v7(),
        };
        registry
            .put_retention(actor, file, &first)
            .await
            .expect("first put");
        registry
            .put_retention(actor, file, &stricter)
            .await
            .expect("overwrite put");
        let got = registry
            .find_retention(actor, file, RetentionScope::OriginalContent)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.deadline, earlier);
    }

    #[tokio::test]
    async fn delete_is_idempotent() {
        let (registry, _dir) = registry();
        let actor = Uuid::now_v7();
        let file = Uuid::now_v7();
        // Delete missing — should succeed silently.
        registry
            .delete_retention(actor, file, RetentionScope::OriginalContent)
            .await
            .expect("delete-missing succeeds");
        // Put, then delete, then delete again.
        let record = RetentionRecord {
            scope: RetentionScope::OriginalContent,
            deadline: now(),
            source_run_id: Uuid::now_v7(),
        };
        registry.put_retention(actor, file, &record).await.unwrap();
        registry
            .delete_retention(actor, file, RetentionScope::OriginalContent)
            .await
            .unwrap();
        registry
            .delete_retention(actor, file, RetentionScope::OriginalContent)
            .await
            .unwrap();
        let got = registry
            .find_retention(actor, file, RetentionScope::OriginalContent)
            .await
            .unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn list_for_file_returns_every_scope() {
        let (registry, _dir) = registry();
        let actor = Uuid::now_v7();
        let file = Uuid::now_v7();
        let run = Uuid::now_v7();
        for scope in [
            RetentionScope::OriginalContent,
            RetentionScope::RedactedOutput,
            RetentionScope::AuditLogs,
        ] {
            let record = RetentionRecord {
                scope,
                deadline: now(),
                source_run_id: run,
            };
            registry.put_retention(actor, file, &record).await.unwrap();
        }
        let rows = registry
            .list_retention_for_file(actor, file)
            .await
            .expect("list succeeds");
        assert_eq!(rows.len(), 3);
        let mut scopes: Vec<RetentionScope> = rows.iter().map(|r| r.scope).collect();
        scopes.sort_by_key(|s| match s {
            RetentionScope::OriginalContent => 0,
            RetentionScope::RedactedOutput => 1,
            RetentionScope::AuditLogs => 2,
            _ => 99,
        });
        assert_eq!(
            scopes,
            vec![
                RetentionScope::OriginalContent,
                RetentionScope::RedactedOutput,
                RetentionScope::AuditLogs,
            ]
        );
    }

    #[tokio::test]
    async fn list_for_file_isolates_by_actor_and_file() {
        let (registry, _dir) = registry();
        let actor_a = Uuid::now_v7();
        let actor_b = Uuid::now_v7();
        let file_x = Uuid::now_v7();
        let file_y = Uuid::now_v7();
        let record = RetentionRecord {
            scope: RetentionScope::OriginalContent,
            deadline: now(),
            source_run_id: Uuid::now_v7(),
        };
        registry
            .put_retention(actor_a, file_x, &record)
            .await
            .unwrap();
        registry
            .put_retention(actor_a, file_y, &record)
            .await
            .unwrap();
        registry
            .put_retention(actor_b, file_x, &record)
            .await
            .unwrap();

        let rows = registry
            .list_retention_for_file(actor_a, file_x)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1, "should not leak across files or actors");
    }

    #[tokio::test]
    async fn list_due_returns_only_expired_rows() {
        let (registry, _dir) = registry();
        let actor = Uuid::now_v7();
        let due_file = Uuid::now_v7();
        let pending_file = Uuid::now_v7();

        let due_record = RetentionRecord {
            scope: RetentionScope::OriginalContent,
            deadline: now().checked_sub(SignedDuration::from_secs(60)).unwrap(),
            source_run_id: Uuid::now_v7(),
        };
        let pending_record = RetentionRecord {
            scope: RetentionScope::OriginalContent,
            deadline: now().checked_add(SignedDuration::from_secs(60)).unwrap(),
            source_run_id: Uuid::now_v7(),
        };
        registry
            .put_retention(actor, due_file, &due_record)
            .await
            .unwrap();
        registry
            .put_retention(actor, pending_file, &pending_record)
            .await
            .unwrap();

        let due = registry
            .list_due_retention(now())
            .await
            .expect("list_due succeeds");
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].actor_id, actor);
        assert_eq!(due[0].file_id, due_file);
        assert_eq!(due[0].record, due_record);
    }

    #[tokio::test]
    async fn list_due_walks_across_actors_and_files() {
        let (registry, _dir) = registry();
        let actor_a = Uuid::now_v7();
        let actor_b = Uuid::now_v7();
        let file = Uuid::now_v7();
        let due = RetentionRecord {
            scope: RetentionScope::OriginalContent,
            deadline: now().checked_sub(SignedDuration::from_secs(1)).unwrap(),
            source_run_id: Uuid::now_v7(),
        };
        registry.put_retention(actor_a, file, &due).await.unwrap();
        registry.put_retention(actor_b, file, &due).await.unwrap();

        let rows = registry.list_due_retention(now()).await.unwrap();
        assert_eq!(rows.len(), 2);
        let mut actors: Vec<Uuid> = rows.iter().map(|r| r.actor_id).collect();
        actors.sort();
        let mut expected = [actor_a, actor_b];
        expected.sort();
        assert_eq!(actors, expected);
    }

    #[tokio::test]
    async fn different_scopes_for_same_file_coexist() {
        let (registry, _dir) = registry();
        let actor = Uuid::now_v7();
        let file = Uuid::now_v7();
        let earlier = now();
        let later = now().checked_add(SignedDuration::from_secs(86400)).unwrap();
        let original = RetentionRecord {
            scope: RetentionScope::OriginalContent,
            deadline: earlier,
            source_run_id: Uuid::now_v7(),
        };
        let redacted = RetentionRecord {
            scope: RetentionScope::RedactedOutput,
            deadline: later,
            source_run_id: Uuid::now_v7(),
        };
        registry
            .put_retention(actor, file, &original)
            .await
            .unwrap();
        registry
            .put_retention(actor, file, &redacted)
            .await
            .unwrap();
        let got_original = registry
            .find_retention(actor, file, RetentionScope::OriginalContent)
            .await
            .unwrap()
            .unwrap();
        let got_redacted = registry
            .find_retention(actor, file, RetentionScope::RedactedOutput)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got_original.deadline, earlier);
        assert_eq!(got_redacted.deadline, later);
    }
}

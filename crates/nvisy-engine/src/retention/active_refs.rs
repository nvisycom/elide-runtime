//! Active-file-reference storage — the sweeper's gate on
//! whether an artifact can safely be deleted.
//!
//! Surfaced as the crate-private [`ActiveFileRefRegistry`]
//! extension trait on [`RegistryHandle`]. Trait is `pub(crate)`:
//! rows are the engine's invariant, written by the run
//! lifecycle at start and cleared at terminal transitions. The
//! sweeper reads them via [`Self::has_active_refs`].
//!
//! ## Keyspace
//!
//! One keyspace, `active_file_refs`, keyed by
//! [`ActiveFileRefKey`] = `(actor_id, file_id, run_id)` (48
//! bytes). Values are empty (`&[]`) — the presence of a row IS
//! the signal.
//!
//! ## Lifecycle coverage
//!
//! - [`Engine::start_run`] inserts one row per input file
//!   before the analyze fan-out begins.
//! - [`Engine::apply_run`], [`Engine::cancel_run`],
//!   [`Engine::delete_run`] each clear every row for the run
//!   at their terminal-state transition.
//! - A crash between run start and terminal transition leaves
//!   orphan rows; the startup reap in [`Engine::open`] clears
//!   any row whose `run_id` no longer exists or points at a
//!   terminal run.
//!
//! [`ActiveFileRefKey`]: crate::registry::ActiveFileRefKey
//! [`Engine::apply_run`]: crate::Engine::apply_run
//! [`Engine::cancel_run`]: crate::Engine::cancel_run
//! [`Engine::delete_run`]: crate::Engine::delete_run
//! [`Engine::open`]: crate::Engine::open
//! [`Engine::start_run`]: crate::Engine::start_run
//! [`RegistryHandle`]: crate::registry::RegistryHandle

use std::error::Error as StdError;

use nvisy_core::{Error, ErrorKind, Result};
use uuid::Uuid;

use crate::Engine;
use crate::registry::{ActiveFileRefKey, RegistryHandle, blocking};
use crate::runs::persist::RunRegistry;

const COMPONENT: &str = "retention::active_refs";

/// Crate-private extension trait adding active-ref storage to
/// [`RegistryHandle`].
pub(crate) trait ActiveFileRefRegistry {
    /// Insert one active-ref row for `(actor, file, run)`.
    /// Idempotent — writing the same triple twice is a no-op.
    fn insert_active_ref(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
        run_id: Uuid,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Delete every active-ref row belonging to `run_id`.
    /// Called at terminal-state transitions
    /// (Applied / PartiallyApplied / Failed / cancelled) with
    /// the run's `document_ids` so each key is synthesised as a
    /// point delete — no scan.
    fn delete_active_refs_for_run(
        &self,
        actor_id: Uuid,
        file_ids: &[Uuid],
        run_id: Uuid,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Whether any active-ref row exists for `(actor, file)`.
    /// The sweeper's gate — returns `true` iff at least one
    /// non-terminal run still references the file. Prefix scan
    /// on `(actor, file)` (32 bytes).
    fn has_active_refs(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
    ) -> impl Future<Output = Result<bool>> + Send;

    /// List every active-ref row across every actor. Used by
    /// the startup reap in [`Engine::open`] to drop orphans
    /// left behind by a crash between run start and terminal
    /// transition.
    ///
    /// [`Engine::open`]: crate::Engine::open
    fn list_all_active_refs(&self) -> impl Future<Output = Result<Vec<ActiveRef>>> + Send;

    /// Delete one specific active-ref row. Used by the startup
    /// reap to drop orphans; the run-side callers use
    /// [`Self::delete_active_refs_for_run`] instead.
    fn delete_active_ref(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
        run_id: Uuid,
    ) -> impl Future<Output = Result<()>> + Send;
}

/// A materialised active-ref row: the three id components. The
/// value slot is empty on disk so no value payload lives here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ActiveRef {
    pub actor_id: Uuid,
    pub file_id: Uuid,
    pub run_id: Uuid,
}

impl ActiveFileRefRegistry for RegistryHandle {
    async fn insert_active_ref(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
        run_id: Uuid,
    ) -> Result<()> {
        let key = ActiveFileRefKey::new(actor_id, file_id, run_id);
        let ks = self.active_file_refs().clone();
        blocking(move || {
            ks.insert(*key, []).map_err(fjall_err)?;
            Ok(())
        })
        .await
    }

    async fn delete_active_refs_for_run(
        &self,
        actor_id: Uuid,
        file_ids: &[Uuid],
        run_id: Uuid,
    ) -> Result<()> {
        // Materialise keys before crossing the blocking
        // boundary so the closure holds no borrows.
        let keys: Vec<ActiveFileRefKey> = file_ids
            .iter()
            .map(|&file_id| ActiveFileRefKey::new(actor_id, file_id, run_id))
            .collect();
        let ks = self.active_file_refs().clone();
        blocking(move || {
            for key in keys {
                ks.remove(*key).map_err(fjall_err)?;
            }
            Ok(())
        })
        .await
    }

    async fn has_active_refs(&self, actor_id: Uuid, file_id: Uuid) -> Result<bool> {
        let ks = self.active_file_refs().clone();
        blocking(move || {
            let prefix = ActiveFileRefKey::file_prefix(actor_id, file_id);
            let mut iter = ks.prefix(prefix);
            match iter.next() {
                Some(guard) => {
                    guard.into_inner().map_err(fjall_err)?;
                    Ok(true)
                }
                None => Ok(false),
            }
        })
        .await
    }

    async fn list_all_active_refs(&self) -> Result<Vec<ActiveRef>> {
        let ks = self.active_file_refs().clone();
        blocking(move || {
            let mut out = Vec::new();
            for guard in ks.iter() {
                let (key, _) = guard.into_inner().map_err(fjall_err)?;
                let (actor_id, file_id, run_id) = ActiveFileRefKey::parse(key.as_ref())
                    .ok_or_else(|| malformed_key_err(key.as_ref()))?;
                out.push(ActiveRef {
                    actor_id,
                    file_id,
                    run_id,
                });
            }
            Ok(out)
        })
        .await
    }

    async fn delete_active_ref(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
        run_id: Uuid,
    ) -> Result<()> {
        let key = ActiveFileRefKey::new(actor_id, file_id, run_id);
        let ks = self.active_file_refs().clone();
        blocking(move || {
            ks.remove(*key).map_err(fjall_err)?;
            Ok(())
        })
        .await
    }
}

impl Engine {
    /// Drop active-file-reference rows left behind by a crash
    /// between run start and terminal transition. Called once
    /// at boot (typically by `nvisy_server::ServiceRuntime::new`)
    /// before the sweeper starts: any row whose `run_id` is
    /// missing or points at a terminal run gets deleted so the
    /// sweeper's gate reflects real active runs, not ghosts.
    ///
    /// Returns the number of rows reaped. Idempotent — running
    /// it twice is safe.
    pub async fn reap_orphan_active_refs(&self) -> Result<usize> {
        let registry = self.registry();
        let refs = registry.list_all_active_refs().await?;
        let mut reaped = 0usize;
        for row in refs {
            let orphan = match registry.get_run(row.actor_id, row.run_id).await {
                Ok(run) => run.state.is_terminal(),
                Err(err) if err.kind() == ErrorKind::NotFound => true,
                Err(err) => {
                    tracing::warn!(
                        target: "engine::reap",
                        actor_id = %row.actor_id,
                        run_id = %row.run_id,
                        file_id = %row.file_id,
                        error = %err,
                        "get_run failed during orphan reap; leaving row in place",
                    );
                    continue;
                }
            };
            if !orphan {
                continue;
            }
            if let Err(err) = registry
                .delete_active_ref(row.actor_id, row.file_id, row.run_id)
                .await
            {
                tracing::warn!(
                    target: "engine::reap",
                    actor_id = %row.actor_id,
                    run_id = %row.run_id,
                    file_id = %row.file_id,
                    error = %err,
                    "failed to delete orphan active-ref row",
                );
                continue;
            }
            reaped += 1;
        }
        Ok(reaped)
    }
}

fn fjall_err(err: impl StdError + Send + Sync + 'static) -> Error {
    Error::internal("fjall operation failed", COMPONENT).with_source(err)
}

fn malformed_key_err(key: &[u8]) -> Error {
    Error::internal(
        format!(
            "active_file_refs key has unexpected length {len}; expected 48",
            len = key.len(),
        ),
        COMPONENT,
    )
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn registry() -> (RegistryHandle, TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let handle = RegistryHandle::open(dir.path()).expect("registry opens");
        (handle, dir)
    }

    #[tokio::test]
    async fn insert_then_gate_sees_row() {
        let (r, _dir) = registry();
        let actor = Uuid::now_v7();
        let file = Uuid::now_v7();
        let run = Uuid::now_v7();
        assert!(!r.has_active_refs(actor, file).await.unwrap());
        r.insert_active_ref(actor, file, run).await.unwrap();
        assert!(r.has_active_refs(actor, file).await.unwrap());
    }

    #[tokio::test]
    async fn insert_is_idempotent() {
        let (r, _dir) = registry();
        let actor = Uuid::now_v7();
        let file = Uuid::now_v7();
        let run = Uuid::now_v7();
        r.insert_active_ref(actor, file, run).await.unwrap();
        r.insert_active_ref(actor, file, run).await.unwrap();
        assert!(r.has_active_refs(actor, file).await.unwrap());
    }

    #[tokio::test]
    async fn delete_run_refs_clears_only_that_run() {
        let (r, _dir) = registry();
        let actor = Uuid::now_v7();
        let file = Uuid::now_v7();
        let run_a = Uuid::now_v7();
        let run_b = Uuid::now_v7();
        r.insert_active_ref(actor, file, run_a).await.unwrap();
        r.insert_active_ref(actor, file, run_b).await.unwrap();
        r.delete_active_refs_for_run(actor, &[file], run_a)
            .await
            .unwrap();
        // Gate still trips because run_b's row survives.
        assert!(r.has_active_refs(actor, file).await.unwrap());
        r.delete_active_refs_for_run(actor, &[file], run_b)
            .await
            .unwrap();
        assert!(!r.has_active_refs(actor, file).await.unwrap());
    }

    #[tokio::test]
    async fn delete_run_refs_walks_every_file() {
        let (r, _dir) = registry();
        let actor = Uuid::now_v7();
        let file_x = Uuid::now_v7();
        let file_y = Uuid::now_v7();
        let run = Uuid::now_v7();
        r.insert_active_ref(actor, file_x, run).await.unwrap();
        r.insert_active_ref(actor, file_y, run).await.unwrap();
        r.delete_active_refs_for_run(actor, &[file_x, file_y], run)
            .await
            .unwrap();
        assert!(!r.has_active_refs(actor, file_x).await.unwrap());
        assert!(!r.has_active_refs(actor, file_y).await.unwrap());
    }

    #[tokio::test]
    async fn gate_is_actor_scoped() {
        let (r, _dir) = registry();
        let actor_a = Uuid::now_v7();
        let actor_b = Uuid::now_v7();
        let file = Uuid::now_v7();
        let run = Uuid::now_v7();
        r.insert_active_ref(actor_a, file, run).await.unwrap();
        assert!(r.has_active_refs(actor_a, file).await.unwrap());
        assert!(
            !r.has_active_refs(actor_b, file).await.unwrap(),
            "active refs must not leak across actors",
        );
    }

    #[tokio::test]
    async fn list_all_returns_every_row_with_parsed_ids() {
        let (r, _dir) = registry();
        let actor_a = Uuid::now_v7();
        let actor_b = Uuid::now_v7();
        let file = Uuid::now_v7();
        let run_a = Uuid::now_v7();
        let run_b = Uuid::now_v7();
        r.insert_active_ref(actor_a, file, run_a).await.unwrap();
        r.insert_active_ref(actor_b, file, run_b).await.unwrap();
        let mut rows = r.list_all_active_refs().await.unwrap();
        rows.sort_by_key(|r| r.actor_id);
        let mut expected = vec![
            ActiveRef {
                actor_id: actor_a,
                file_id: file,
                run_id: run_a,
            },
            ActiveRef {
                actor_id: actor_b,
                file_id: file,
                run_id: run_b,
            },
        ];
        expected.sort_by_key(|r| r.actor_id);
        assert_eq!(rows, expected);
    }

    #[tokio::test]
    async fn delete_active_ref_is_idempotent() {
        let (r, _dir) = registry();
        let actor = Uuid::now_v7();
        let file = Uuid::now_v7();
        let run = Uuid::now_v7();
        // Delete missing row — succeeds.
        r.delete_active_ref(actor, file, run).await.unwrap();
        r.insert_active_ref(actor, file, run).await.unwrap();
        r.delete_active_ref(actor, file, run).await.unwrap();
        r.delete_active_ref(actor, file, run).await.unwrap();
        assert!(!r.has_active_refs(actor, file).await.unwrap());
    }
}

#[cfg(test)]
mod reap_tests {
    use nvisy_core::plan::AnalyzerParams;
    use tempfile::TempDir;

    use super::*;
    use crate::runs::{Run, RunState};

    fn engine() -> (Engine, TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = Engine::open(dir.path()).expect("engine opens");
        (engine, dir)
    }

    #[tokio::test]
    async fn reap_drops_row_for_missing_run() {
        // Plant an active-ref row whose run_id doesn't
        // correspond to any run header — simulates a crash that
        // deleted the header but left the ref behind.
        let (engine, _dir) = engine();
        let actor = Uuid::now_v7();
        let file = Uuid::now_v7();
        let ghost_run = Uuid::now_v7();
        engine
            .registry()
            .insert_active_ref(actor, file, ghost_run)
            .await
            .unwrap();
        assert!(engine.has_active_refs(actor, file).await.unwrap());

        let reaped = engine.reap_orphan_active_refs().await.unwrap();
        assert_eq!(reaped, 1);
        assert!(!engine.has_active_refs(actor, file).await.unwrap());
    }

    #[tokio::test]
    async fn reap_drops_row_for_terminal_run() {
        // Fabricate a terminal run header + plant a fresh ref
        // for it — simulates a crash between the terminal state
        // write and the active-ref clear inside apply_run /
        // cancel_run / delete_run.
        let (engine, _dir) = engine();
        let actor = Uuid::now_v7();
        let file = Uuid::now_v7();
        let run_id = Uuid::now_v7();
        let now = jiff::Timestamp::now();
        let run = Run {
            id: run_id,
            state: RunState::Failed {
                reason: "test".to_owned(),
            },
            started_at: now,
            updated_at: now,
            policy_refs: Vec::new(),
            context_refs: Vec::new(),
            metadata: Default::default(),
            document_ids: Vec::new(),
            analyzer: AnalyzerParams::default(),
            concurrency: 1,
        };
        engine.registry().put_run(actor, &run).await.unwrap();
        engine
            .registry()
            .insert_active_ref(actor, file, run_id)
            .await
            .unwrap();
        assert!(engine.has_active_refs(actor, file).await.unwrap());

        let reaped = engine.reap_orphan_active_refs().await.unwrap();
        assert_eq!(reaped, 1);
        assert!(!engine.has_active_refs(actor, file).await.unwrap());
    }
}

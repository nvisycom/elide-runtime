//! Retention sweeper: the background task that walks the
//! retention schedule and deletes artifacts whose deadline has
//! passed.
//!
//! Two public entry points, both hanging off [`Engine`]:
//!
//! - [`Engine::sweep_once`] runs one pass and returns a
//!   [`SweepReport`]. Suitable for on-demand sweeps in tests
//!   and admin tooling.
//! - [`Engine::start_sweeper`] spawns a background task that
//!   loops on `sweep_once` at a fixed interval. Returns a
//!   [`SweeperHandle`] whose [`SweeperHandle::stop`] cancels
//!   the loop and awaits the join.
//!
//! ## Per-tick behaviour
//!
//! 1. Enumerate every retention row whose `deadline <= now`
//!    ([`super::retention::RetentionRegistry::list_due_retention`]).
//! 2. For each due row:
//!    - `OriginalContent` scope: check
//!      [`super::active_refs::ActiveFileRefRegistry::has_active_refs`].
//!      If any non-terminal run still references the file,
//!      defer — the row stays; the next tick reconsiders.
//!    - `RedactedOutput` scope: no active-run gate (no run
//!      ever reads its own output back), delete unconditionally.
//!    - `AuditLogs` scope: skipped today (the audit resource
//!      lands in phase 5); the row stays until that keyspace
//!      exists.
//! 3. Delete the file blob via
//!    [`crate::FileRegistry::delete_file`]. If it returns
//!    `NotFound` the artifact is already gone (concurrent
//!    delete, or a partial previous sweep) — treat as swept.
//! 4. Delete the retention row.
//!
//! A per-row error logs a warn and moves on; one bad file
//! never stalls the sweep.
//!
//! [`Engine`]: crate::Engine
//! [`Engine::sweep_once`]: crate::Engine::sweep_once
//! [`Engine::start_sweeper`]: crate::Engine::start_sweeper

use std::time::Duration;

use jiff::Timestamp;
use nvisy_core::{ErrorKind, Result};
use nvisy_schema::policy::RetentionScope;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::active_refs::ActiveFileRefRegistry;
use super::schedule::{DueRetention, RetentionRegistry};
use crate::registry::RegistryHandle;
use crate::{Engine, FileRegistry};

/// Counts for one sweep pass. Every due row lands in exactly
/// one bucket.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SweepReport {
    /// File deleted (or already gone) and the retention row
    /// dropped.
    pub swept: usize,
    /// File held back because an active run still references it
    /// (`OriginalContent`-only path), or the scope has no target
    /// yet (`AuditLogs` pending phase 5).
    pub deferred: usize,
    /// File-delete or row-delete errored. The row stays; the
    /// next tick retries.
    pub errored: usize,
}

/// Owns the spawned sweeper task. Drop it to abandon (the task
/// keeps running to completion of the current tick, then stops
/// on the next `select!`); call [`Self::stop`] to cancel and
/// await deterministic shutdown.
pub struct SweeperHandle {
    cancel: CancellationToken,
    task: JoinHandle<()>,
}

impl SweeperHandle {
    /// Cancel the sweeper loop and await the task. Idempotent
    /// (dropping without stopping is fine, but you lose the
    /// deterministic shutdown moment).
    pub async fn stop(self) {
        self.cancel.cancel();
        let _ = self.task.await;
    }
}

impl Engine {
    /// One sweep pass. Takes an explicit `now` so tests can
    /// inject a deadline without waiting real time; the
    /// background loop passes [`Timestamp::now`].
    pub async fn sweep_once(&self, now: Timestamp) -> Result<SweepReport> {
        let registry = self.registry();
        let due = registry.list_due_retention(now).await?;
        let mut report = SweepReport::default();
        for row in due {
            match sweep_row(registry, &row).await {
                RowOutcome::Swept => report.swept += 1,
                RowOutcome::Deferred => report.deferred += 1,
                RowOutcome::Errored => report.errored += 1,
            }
        }
        Ok(report)
    }

    /// Spawn the periodic sweeper. The task ticks every
    /// `interval` and shuts down on the returned handle's
    /// [`SweeperHandle::stop`].
    pub fn start_sweeper(&self, interval: Duration) -> SweeperHandle {
        let cancel = CancellationToken::new();
        let child = cancel.child_token();
        let engine = self.clone();
        let task = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            // `Delay` so we don't burn ticks when a slow sweep
            // overruns; the first `tick()` fires immediately.
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    biased;
                    _ = child.cancelled() => break,
                    _ = ticker.tick() => {
                        match engine.sweep_once(Timestamp::now()).await {
                            Ok(report) => {
                                if report.swept + report.deferred + report.errored > 0 {
                                    tracing::debug!(
                                        target: "engine::sweeper",
                                        swept = report.swept,
                                        deferred = report.deferred,
                                        errored = report.errored,
                                        "sweep tick",
                                    );
                                }
                            }
                            Err(err) => {
                                tracing::warn!(
                                    target: "engine::sweeper",
                                    error = %err,
                                    "sweep_once failed; will retry next tick",
                                );
                            }
                        }
                    }
                }
            }
        });
        SweeperHandle { cancel, task }
    }
}

enum RowOutcome {
    Swept,
    Deferred,
    Errored,
}

async fn sweep_row(registry: &RegistryHandle, row: &DueRetention) -> RowOutcome {
    match row.record.scope {
        RetentionScope::AuditLogs => {
            // The audit keyspace lands in phase 5; nothing to
            // delete yet. Defer so the row survives until the
            // sweeper knows what to do with it.
            RowOutcome::Deferred
        }
        RetentionScope::OriginalContent => {
            match registry.has_active_refs(row.actor_id, row.file_id).await {
                Ok(true) => RowOutcome::Deferred,
                Ok(false) => delete_and_clear(registry, row).await,
                Err(err) => {
                    log_row_error(row, "has_active_refs", &err);
                    RowOutcome::Errored
                }
            }
        }
        RetentionScope::RedactedOutput => delete_and_clear(registry, row).await,
        // `RetentionScope` is `#[non_exhaustive]`. A future
        // variant reaching this arm should defer so the row
        // survives for a binary that knows what to do with it.
        _ => RowOutcome::Deferred,
    }
}

async fn delete_and_clear(registry: &RegistryHandle, row: &DueRetention) -> RowOutcome {
    match registry.delete_file(row.actor_id, row.file_id).await {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {
            // File already gone (concurrent delete, or a
            // partial previous sweep). Treat as swept and
            // drop the row.
        }
        Err(err) => {
            log_row_error(row, "delete_file", &err);
            return RowOutcome::Errored;
        }
    }
    match registry
        .delete_retention(row.actor_id, row.file_id, row.record.scope)
        .await
    {
        Ok(()) => RowOutcome::Swept,
        Err(err) => {
            log_row_error(row, "delete_retention", &err);
            RowOutcome::Errored
        }
    }
}

fn log_row_error(row: &DueRetention, op: &str, err: &nvisy_core::Error) {
    tracing::warn!(
        target: "engine::sweeper",
        actor_id = %row.actor_id,
        file_id = %row.file_id,
        scope = ?row.record.scope,
        source_run_id = %row.record.source_run_id,
        op,
        error = %err,
        "sweep_row failed",
    );
}

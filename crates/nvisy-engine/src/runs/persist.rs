//! fjall read/write for run state.
//!
//! Surfaced as the crate-private [`RunRegistry`] extension trait
//! on [`RegistryHandle`] so the orchestrator reaches storage the
//! same way the public resource APIs do — `registry.put_run(...)`,
//! `registry.get_run_doc(...)`, etc. Trait is `pub(crate)`: a run
//! is the orchestrator's invariant, so external code should never
//! manipulate it directly; the public surface is
//! [`crate::Engine::start_run`] / [`crate::Engine::apply_run`]
//! and their siblings.
//!
//! Two keyspaces, both keyed by `(actor_id, run_id, …)`:
//!
//! - **run_headers** — [`CompositeKey(actor, run)`] holding the
//!   [`Run`] metadata blob (state, refs, timestamps).
//! - **run_docs** — [`RunDocKey(actor, run, doc)`] holding the
//!   per-document body (entities + overrides + `input_file_id`
//!   + `output_file_id`).
//!
//! Bytes (inputs + redacted outputs) live in the
//! [`crate::FileRegistry`], not in any run keyspace. The
//! per-doc row points at them by id.
//!
//! [`CompositeKey(actor, run)`]: crate::registry::CompositeKey
//! [`RunDocKey(actor, run, doc)`]: crate::registry::RunDocKey

use std::error::Error as StdError;

use nvisy_core::{Error, Result};
use uuid::Uuid;

use super::state::{Run, RunDocument};
use crate::registry::{CompositeKey, RegistryHandle, RunDocKey, blocking, not_found};

const COMPONENT: &str = "runs::persist";
const KIND_RUN: &str = "run";
const KIND_DOC: &str = "run_document";

/// Crate-private extension trait adding run-lifecycle storage to
/// [`RegistryHandle`]. Consumed exclusively by the orchestrator
/// and the public reader functions in [`crate::runs`].
pub(crate) trait RunRegistry {
    /// Write the run header at `(actor_id, run.id)`. Overwrites
    /// any existing header — state transitions go through this.
    fn put_run(&self, actor_id: Uuid, run: &Run) -> impl Future<Output = Result<()>> + Send;

    /// Read the run header at `(actor_id, run_id)`.
    fn get_run(&self, actor_id: Uuid, run_id: Uuid) -> impl Future<Output = Result<Run>> + Send;

    /// List every run header for `actor_id`. Returns full
    /// headers; callers filter by [`super::state::RunState`].
    fn list_runs(&self, actor_id: Uuid) -> impl Future<Output = Result<Vec<Run>>> + Send;

    /// Delete the run header at `(actor_id, run_id)`. Caller
    /// drives the per-doc cascade via
    /// [`delete_run_bodies`](Self::delete_run_bodies) first.
    fn delete_run(&self, actor_id: Uuid, run_id: Uuid) -> impl Future<Output = Result<()>> + Send;

    /// Write a per-doc body at `(actor_id, run_id, doc.id)`.
    /// Overwrites; reviewer overrides and per-doc state
    /// transitions flow through this.
    fn put_run_doc(
        &self,
        actor_id: Uuid,
        run_id: Uuid,
        doc: &RunDocument,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Read a per-doc body at `(actor_id, run_id, doc_id)`.
    fn get_run_doc(
        &self,
        actor_id: Uuid,
        run_id: Uuid,
        doc_id: Uuid,
    ) -> impl Future<Output = Result<RunDocument>> + Send;

    /// Remove every per-doc body under `(actor_id, run_id)` in
    /// `run_docs`. Returns the number of rows removed. Does not
    /// touch the input or output files in [`FileRegistry`] —
    /// per the no-cascade-delete contract, file lifecycle is
    /// independent of run lifecycle.
    ///
    /// [`FileRegistry`]: crate::FileRegistry
    fn delete_run_bodies(
        &self,
        actor_id: Uuid,
        run_id: Uuid,
    ) -> impl Future<Output = Result<usize>> + Send;
}

impl RunRegistry for RegistryHandle {
    async fn put_run(&self, actor_id: Uuid, run: &Run) -> Result<()> {
        let key = CompositeKey::new(actor_id, run.id);
        let value = serde_json::to_vec(run)?;
        let headers = self.run_headers().clone();
        blocking(move || {
            headers.insert(*key, value).map_err(fjall_err)?;
            Ok(())
        })
        .await
    }

    async fn get_run(&self, actor_id: Uuid, run_id: Uuid) -> Result<Run> {
        let key = CompositeKey::new(actor_id, run_id);
        let headers = self.run_headers().clone();
        blocking(move || {
            let value = headers
                .get(*key)
                .map_err(fjall_err)?
                .ok_or_else(|| not_found(KIND_RUN, actor_id, run_id))?;
            serde_json::from_slice(&value).map_err(Into::into)
        })
        .await
    }

    async fn list_runs(&self, actor_id: Uuid) -> Result<Vec<Run>> {
        let headers = self.run_headers().clone();
        blocking(move || {
            let prefix = CompositeKey::actor_prefix(actor_id);
            let mut out: Vec<Run> = Vec::new();
            for guard in headers.prefix(prefix) {
                let (_, value) = guard.into_inner().map_err(fjall_err)?;
                let run: Run = serde_json::from_slice(&value)?;
                out.push(run);
            }
            Ok(out)
        })
        .await
    }

    async fn delete_run(&self, actor_id: Uuid, run_id: Uuid) -> Result<()> {
        let key = CompositeKey::new(actor_id, run_id);
        let headers = self.run_headers().clone();
        blocking(move || {
            headers.remove(*key).map_err(fjall_err)?;
            Ok(())
        })
        .await
    }

    async fn put_run_doc(&self, actor_id: Uuid, run_id: Uuid, doc: &RunDocument) -> Result<()> {
        let key = RunDocKey::new(actor_id, run_id, doc.id);
        let value = serde_json::to_vec(doc)?;
        let docs = self.run_docs().clone();
        blocking(move || {
            docs.insert(*key, value).map_err(fjall_err)?;
            Ok(())
        })
        .await
    }

    async fn get_run_doc(&self, actor_id: Uuid, run_id: Uuid, doc_id: Uuid) -> Result<RunDocument> {
        let key = RunDocKey::new(actor_id, run_id, doc_id);
        let docs = self.run_docs().clone();
        blocking(move || {
            let value = docs
                .get(*key)
                .map_err(fjall_err)?
                .ok_or_else(|| not_found(KIND_DOC, actor_id, doc_id))?;
            serde_json::from_slice(&value).map_err(Into::into)
        })
        .await
    }

    async fn delete_run_bodies(&self, actor_id: Uuid, run_id: Uuid) -> Result<usize> {
        let docs = self.run_docs().clone();
        blocking(move || {
            let prefix = RunDocKey::run_prefix(actor_id, run_id);
            // Collect first, then remove — iterating while
            // mutating a fjall keyspace is unsupported.
            let keys: Vec<Vec<u8>> = docs
                .prefix(prefix)
                .map(|guard| {
                    let key = guard.key().map_err(fjall_err)?;
                    Ok(key.as_ref().to_vec())
                })
                .collect::<Result<Vec<_>>>()?;
            let removed = keys.len();
            for key in keys {
                docs.remove(&key).map_err(fjall_err)?;
            }
            Ok(removed)
        })
        .await
    }
}

fn fjall_err(err: impl StdError + Send + Sync + 'static) -> Error {
    Error::internal("fjall operation failed", COMPONENT).with_source(err)
}

//! fjall read/write helpers for [`Run`] headers, [`RunDocument`]
//! bodies, and post-apply artifact bytes.
//!
//! Three keyspaces, all on the same [`RegistryHandle`]:
//!
//! - **run_headers** (`CompositeKey(actor, run)`) — short JSON
//!   metadata describing the run.
//! - **run_docs** (`TripleKey(actor, run, doc)`) — JSON body per
//!   document carrying recognized entities + reviewer overrides.
//! - **run_artifacts** (`TripleKey(actor, run, doc)`) — raw bytes
//!   of the post-apply redacted file. Lazy: only loaded when the
//!   caller asks for the redacted output.

use bytes::Bytes;
use nvisy_core::{Error, Result};
use uuid::Uuid;

use crate::registry::{CompositeKey, RegistryHandle, TripleKey, blocking, not_found};

use super::state::{Run, RunDocument};

const COMPONENT: &str = "runs::persist";
const KIND_RUN: &str = "run";
const KIND_DOC: &str = "run_document";

/// Write the run header at `(actor_id, run.id)`. Overwrites any
/// existing header (state transitions go through this).
pub(crate) async fn put_header(
    handle: &RegistryHandle,
    actor_id: Uuid,
    run: &Run,
) -> Result<()> {
    let key = CompositeKey::new(actor_id, run.id);
    let value = serde_json::to_vec(run)?;
    let headers = handle.run_headers().clone();
    blocking(move || {
        headers.insert(*key, value).map_err(fjall_err)?;
        Ok(())
    })
    .await
}

/// Read the run header at `(actor_id, run_id)`.
pub async fn get_header(
    handle: &RegistryHandle,
    actor_id: Uuid,
    run_id: Uuid,
) -> Result<Run> {
    let key = CompositeKey::new(actor_id, run_id);
    let headers = handle.run_headers().clone();
    blocking(move || {
        let value = headers
            .get(*key)
            .map_err(fjall_err)?
            .ok_or_else(|| not_found(KIND_RUN, actor_id, run_id))?;
        serde_json::from_slice(&value).map_err(Into::into)
    })
    .await
}

/// Write a per-doc body at `(actor_id, run_id, doc_id)`.
/// Overwrites; reviewer overrides and per-doc state transitions
/// flow through this.
pub(crate) async fn put_doc(
    handle: &RegistryHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc: &RunDocument,
) -> Result<()> {
    let key = TripleKey::new(actor_id, run_id, doc.id);
    let value = serde_json::to_vec(doc)?;
    let docs = handle.run_docs().clone();
    blocking(move || {
        docs.insert(*key, value).map_err(fjall_err)?;
        Ok(())
    })
    .await
}

/// Read a per-doc body at `(actor_id, run_id, doc_id)`.
pub async fn get_doc(
    handle: &RegistryHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
) -> Result<RunDocument> {
    let key = TripleKey::new(actor_id, run_id, doc_id);
    let docs = handle.run_docs().clone();
    blocking(move || {
        let value = docs
            .get(*key)
            .map_err(fjall_err)?
            .ok_or_else(|| not_found(KIND_DOC, actor_id, doc_id))?;
        serde_json::from_slice(&value).map_err(Into::into)
    })
    .await
}

/// Write the post-apply redacted bytes for one doc.
pub(crate) async fn put_artifact(
    handle: &RegistryHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
    bytes: Bytes,
) -> Result<()> {
    let key = TripleKey::new(actor_id, run_id, doc_id);
    let artifacts = handle.run_artifacts().clone();
    blocking(move || {
        artifacts.insert(*key, bytes.to_vec()).map_err(fjall_err)?;
        Ok(())
    })
    .await
}

/// Read the post-apply redacted bytes for one doc. Returns
/// [`ErrorKind::NotFound`] when no artifact has been written
/// (i.e. apply did not run or the doc failed).
///
/// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
pub async fn get_artifact(
    handle: &RegistryHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
) -> Result<Bytes> {
    let key = TripleKey::new(actor_id, run_id, doc_id);
    let artifacts = handle.run_artifacts().clone();
    blocking(move || {
        let value = artifacts
            .get(*key)
            .map_err(fjall_err)?
            .ok_or_else(|| not_found("run_artifact", actor_id, doc_id))?;
        Ok(Bytes::from(value.to_vec()))
    })
    .await
}

/// Write the original input bytes for one doc.
pub(crate) async fn put_input(
    handle: &RegistryHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
    bytes: Bytes,
) -> Result<()> {
    let key = TripleKey::new(actor_id, run_id, doc_id);
    let inputs = handle.run_inputs().clone();
    blocking(move || {
        inputs.insert(*key, bytes.to_vec()).map_err(fjall_err)?;
        Ok(())
    })
    .await
}

/// Read the original input bytes for one doc.
pub(crate) async fn get_input(
    handle: &RegistryHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
) -> Result<Bytes> {
    let key = TripleKey::new(actor_id, run_id, doc_id);
    let inputs = handle.run_inputs().clone();
    blocking(move || {
        let value = inputs
            .get(*key)
            .map_err(fjall_err)?
            .ok_or_else(|| not_found("run_input", actor_id, doc_id))?;
        Ok(Bytes::from(value.to_vec()))
    })
    .await
}

fn fjall_err(err: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::internal("fjall operation failed", COMPONENT).with_source(err)
}

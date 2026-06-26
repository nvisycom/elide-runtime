//! Run lifecycle orchestration: [`start`] resolves input files,
//! writes Queued per-doc rows, fans the analyzer out, and flips
//! the run into [`RunState::AwaitingReview`]. [`apply`] runs the
//! per-doc anonymizer pass, writes the redacted bytes back to
//! the [`FileRegistry`] as new files (stamped with
//! [`FileLineage::RedactedFrom`]), records each output's id on
//! the per-doc row, and flips the run into [`RunState::Applied`]
//! or [`RunState::PartiallyApplied`].
//!
//! [`FileRegistry`]: crate::FileRegistry
//! [`FileLineage::RedactedFrom`]: nvisy_core::FileLineage::RedactedFrom
//!
//! Fan-out shape (shared by analyze and apply):
//!
//! - [`futures::stream::iter`] over the per-doc workload,
//!   capped at [`Run::concurrency`] in-flight by
//!   [`StreamExt::buffer_unordered`].
//! - Each per-doc future is wrapped in [`tokio::time::timeout`]
//!   so a single stuck recognizer never stalls the run.
//! - Per-doc failures (validation, timeout) are recorded in the
//!   per-doc [`RunDocState`]; they do not fail the run as a
//!   whole — apply will only run for docs that reached
//!   [`RunDocState::AwaitingReview`].
//!
//! Stream-based concurrency (not [`tokio::task::JoinSet`]) is
//! deliberate: each per-doc future borrows the [`EngineHandle`]
//! and the analyzer spec, and `JoinSet::spawn` would force them
//! to `'static`. The stream combinator keeps the borrows live
//! for free.
//!
//! [`EngineHandle`]: crate::EngineHandle

use std::time::Duration;

use futures::StreamExt;
use jiff::Timestamp;
use nvisy_core::file::FileMetadata;
use nvisy_core::{FileLineage, Result};
use uuid::Uuid;

use super::filter::{DocumentFacts, merge_metadata};
use super::input::StartBatch;
use super::persist::RunRegistry;
use super::pipeline::{analyze_document, apply_document};
use super::state::{DocBody, ModalityKind, Run, RunDocState, RunDocument, RunState};
use crate::keyspace::FileDescriptor;
use crate::{EngineHandle, FileRegistry, PolicyRegistry};

/// Default per-run concurrency cap when the caller's
/// [`StartBatch::concurrency`] is `None`.
const DEFAULT_CONCURRENCY: usize = 4;

/// Per-doc analyze hard timeout. Recognizers that exceed this
/// land in [`RunDocState::TimedOut`]; the rest of the run
/// continues.
const PER_DOC_TIMEOUT: Duration = Duration::from_secs(120);

/// Start a run.
///
/// Mints a UUIDv7 run id and per-doc ids, persists the input
/// bytes together with a Queued per-doc row for every input,
/// writes the run header in [`RunState::Analyzing`], then fans
/// the analyzer out across docs. Each task rewrites its per-doc
/// row with the recognized entities and new state. When the
/// fan-out finishes, the header flips to
/// [`RunState::AwaitingReview`].
///
/// Returns the new run id; the per-doc results are queryable via
/// [`super::get_doc`].
pub async fn start(engine: &EngineHandle, actor_id: Uuid, batch: StartBatch) -> Result<Uuid> {
    let run_id = Uuid::now_v7();
    let now = Timestamp::now();
    let concurrency = batch.concurrency.unwrap_or(DEFAULT_CONCURRENCY).max(1);

    // Mint a doc id per input and write a Queued per-doc row up
    // front so the run is fully queryable the moment the header
    // lands. `modality` defaults to Text; the analyzer pass
    // overwrites it once the codec resolves. `body` stays empty
    // until analyze finishes.
    let registry = engine.registry();
    let mut document_ids = Vec::with_capacity(batch.documents.len());
    for doc in &batch.documents {
        let doc_id = Uuid::now_v7();
        document_ids.push(doc_id);

        let modality = ModalityKind::Text;
        let document = RunDocument {
            id: doc_id,
            input_file_id: doc.file_id,
            output_file_id: None,
            state: RunDocState::Queued,
            modality,
            body: DocBody::empty(modality),
        };
        registry.put_run_doc(actor_id, run_id, &document).await?;
    }

    let run = Run {
        id: run_id,
        state: RunState::Analyzing,
        started_at: now,
        updated_at: now,
        policy_refs: batch.policy_refs,
        context_refs: batch.context_refs,
        metadata: batch.metadata,
        document_ids: document_ids.clone(),
        analyzer: batch.analyzer,
        concurrency,
    };
    registry.put_run(actor_id, &run).await?;

    // Materialise (doc_id, file_id) pairs as owned `Copy`
    // tuples; closures clone the analyzer spec per item so the
    // resulting futures hold no borrows on this stack frame —
    // important when this future is consumed from an axum
    // handler where the outer future must be `Send + 'static`.
    let analyze_inputs: Vec<(Uuid, Uuid)> = document_ids
        .iter()
        .zip(batch.documents.iter())
        .map(|(doc_id, doc)| (*doc_id, doc.file_id))
        .collect();
    let analyzer_spec = run.analyzer.clone();
    let work = analyze_inputs.into_iter().map(|(doc_id, file_id)| {
        analyze_one_doc(
            engine,
            actor_id,
            run_id,
            doc_id,
            file_id,
            analyzer_spec.clone(),
        )
    });
    futures::stream::iter(work)
        .buffer_unordered(concurrency)
        .for_each(|()| async {})
        .await;

    // All per-doc rows have settled in their terminal analyze
    // state (AwaitingReview / Failed / TimedOut); flip the header
    // so callers see the run is ready for review.
    let mut header = run;
    header.state = RunState::AwaitingReview;
    header.updated_at = Timestamp::now();
    registry.put_run(actor_id, &header).await?;

    Ok(run_id)
}

/// One per-doc unit of the analyze fan-out. Owns the lifecycle
/// transitions for one row; never returns an error (failures
/// land in the row's state).
///
/// `spec` is owned (not borrowed) so the resulting future
/// captures no references on its caller's stack — keeps the
/// outer `runs::start` future composable with handler-level
/// `Send + 'static` bounds.
async fn analyze_one_doc(
    engine: &EngineHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
    input_file_id: Uuid,
    spec: nvisy_core::plan::AnalyzerSpec,
) {
    let registry = engine.registry();
    mark_analyzing(registry, actor_id, run_id, doc_id).await;

    let outcome = match load_input(registry, actor_id, input_file_id).await {
        Ok((file, bytes)) => {
            let analyze = analyze_document(engine.formats(), bytes, file.extension.as_str(), &spec);
            match tokio::time::timeout(PER_DOC_TIMEOUT, analyze).await {
                Ok(Ok(outcome)) => Ok(outcome),
                Ok(Err(err)) => Err(DocFailure::Failed(err.to_string())),
                Err(_) => Err(DocFailure::TimedOut),
            }
        }
        Err(err) => Err(DocFailure::Failed(err.to_string())),
    };

    write_outcome(registry, actor_id, run_id, doc_id, outcome).await;
}

/// Fetch the file metadata + bytes for one input file. Two
/// reads (metadata then content) because the file API splits
/// them across keyspaces.
async fn load_input(
    registry: &crate::registry::RegistryHandle,
    actor_id: Uuid,
    file_id: Uuid,
) -> Result<(FileMetadata, bytes::Bytes)> {
    let file = registry.get_file(actor_id, file_id).await?;
    let bytes = registry.get_file_bytes(actor_id, file_id).await?;
    Ok((file, bytes))
}

enum DocFailure {
    Failed(String),
    TimedOut,
}

async fn mark_analyzing(
    registry: &crate::registry::RegistryHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
) {
    let Ok(mut doc) = registry.get_run_doc(actor_id, run_id, doc_id).await else {
        return;
    };
    doc.state = RunDocState::Analyzing;
    let _ = registry.put_run_doc(actor_id, run_id, &doc).await;
}

async fn write_outcome(
    registry: &crate::registry::RegistryHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
    outcome: std::result::Result<super::pipeline::AnalyzeOutcome, DocFailure>,
) {
    let Ok(mut doc) = registry.get_run_doc(actor_id, run_id, doc_id).await else {
        return;
    };
    match outcome {
        Ok(out) => {
            doc.modality = out.modality;
            doc.body = out.body;
            doc.state = RunDocState::AwaitingReview;
        }
        Err(DocFailure::Failed(reason)) => {
            doc.state = RunDocState::Failed { reason };
        }
        Err(DocFailure::TimedOut) => {
            doc.state = RunDocState::TimedOut;
        }
    }
    let _ = registry.put_run_doc(actor_id, run_id, &doc).await;
}

/// Apply a run.
///
/// Loads the run header, verifies it is in
/// [`RunState::AwaitingReview`], resolves every referenced
/// policy, fans the per-doc anonymizer pass out under the run's
/// concurrency cap, persists the redacted bytes as artifacts,
/// and rewrites the header to [`RunState::Applied`] (every doc
/// applied) or [`RunState::PartiallyApplied`] (at least one
/// failed).
pub async fn apply(engine: &EngineHandle, actor_id: Uuid, run_id: Uuid) -> Result<()> {
    let registry = engine.registry();
    let mut run = registry.get_run(actor_id, run_id).await?;
    if !matches!(run.state, RunState::AwaitingReview) {
        return Err(nvisy_core::Error::conflict(
            format!(
                "run {run_id} is in state {:?}; apply only valid from AwaitingReview",
                run.state,
            ),
            "runs::apply",
        ));
    }

    // Resolve every referenced policy up-front. Apply needs the
    // full set so each per-doc task can filter by
    // `Policy::applies_when` against that doc's facts.
    let policies =
        std::sync::Arc::new(resolve_policies(registry, actor_id, &run.policy_refs).await?);

    // Materialise doc ids as owned `Copy` values; clone the
    // shared inputs (analyzer spec + metadata + Arc<policies>)
    // into each closure so the stream futures hold no borrows
    // on this stack frame — keeps the outer apply future
    // composable with handler-level `Send + 'static` bounds.
    let doc_ids: Vec<Uuid> = run.document_ids.clone();
    let request_metadata = run.metadata.clone();
    let analyzer_spec = run.analyzer.clone();
    let concurrency = run.concurrency;
    let work = doc_ids.into_iter().map(|doc_id| {
        apply_one_doc(
            engine,
            actor_id,
            run_id,
            doc_id,
            request_metadata.clone(),
            analyzer_spec.clone(),
            std::sync::Arc::clone(&policies),
        )
    });
    let outcomes: Vec<bool> = futures::stream::iter(work)
        .buffer_unordered(concurrency)
        .collect()
        .await;

    let all_applied = outcomes.iter().all(|ok| *ok);
    run.state = if all_applied {
        RunState::Applied
    } else {
        RunState::PartiallyApplied
    };
    run.updated_at = Timestamp::now();
    registry.put_run(actor_id, &run).await?;
    Ok(())
}

/// Read the run header at `(actor_id, run_id)`. Public
/// read-side counterpart to the `pub(crate)`
/// [`RunRegistry::get_run`]; external API consumers reach run
/// state through this rather than the persistence trait.
///
/// [`RunRegistry::get_run`]: super::persist::RunRegistry::get_run
pub async fn get(engine: &EngineHandle, actor_id: Uuid, run_id: Uuid) -> Result<Run> {
    engine.registry().get_run(actor_id, run_id).await
}

/// Read a per-doc body at `(actor_id, run_id, doc_id)`.
pub async fn get_doc(
    engine: &EngineHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
) -> Result<RunDocument> {
    engine
        .registry()
        .get_run_doc(actor_id, run_id, doc_id)
        .await
}

/// List every run for `actor_id`. Returns full headers; callers
/// filter by [`RunState`] (e.g. the HTTP layer's `/detections`
/// view shows non-applied runs, `/redactions` shows applied
/// runs).
pub async fn list(engine: &EngineHandle, actor_id: Uuid) -> Vec<Run> {
    engine
        .registry()
        .list_runs(actor_id)
        .await
        .unwrap_or_default()
}

/// Cancel a run.
///
/// Marks the header [`RunState::Failed`] with `reason = "cancelled"`.
/// Cancel is only valid from [`RunState::Analyzing`] or
/// [`RunState::AwaitingReview`]; calling it on a terminal run
/// ([`RunState::Applied`] / [`RunState::PartiallyApplied`] /
/// [`RunState::Failed`]) returns [`ErrorKind::Conflict`].
///
/// Today's cancel is a *header* operation: in-flight per-doc
/// fan-out tasks are not interrupted; they finish their current
/// step and write their outcome into a doc row no one will read.
/// Cooperative interruption needs a [`tokio_util::sync::CancellationToken`]
/// threaded through the pipeline and lands as a follow-up slice.
///
/// [`ErrorKind::Conflict`]: nvisy_core::ErrorKind::Conflict
/// [`tokio_util::sync::CancellationToken`]: https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html
pub async fn cancel(engine: &EngineHandle, actor_id: Uuid, run_id: Uuid) -> Result<()> {
    let registry = engine.registry();
    let mut run = registry.get_run(actor_id, run_id).await?;
    match run.state {
        RunState::Analyzing | RunState::AwaitingReview => {}
        ref other => {
            return Err(nvisy_core::Error::conflict(
                format!(
                    "run {run_id} is in state {other:?}; cancel only valid from Analyzing or AwaitingReview"
                ),
                "runs::cancel",
            ));
        }
    }
    run.state = RunState::Failed {
        reason: "cancelled".to_owned(),
    };
    run.updated_at = Timestamp::now();
    registry.put_run(actor_id, &run).await
}

/// Delete a run, cascading across all four run keyspaces.
///
/// Removes the header (`run_headers`) plus every per-doc body
/// (`run_docs`), artifact (`run_artifacts`), and input
/// (`run_inputs`) belonging to the run. Returns
/// [`ErrorKind::NotFound`] when the header is missing.
///
/// In-flight cancellation is **not** part of delete; callers
/// should [`cancel`] first when the run is still active, then
/// delete. Calling `delete` on an active run is allowed —
/// per-doc tasks that complete after the delete write into
/// keyspaces that no longer have a header, and are reaped on the
/// next list scan.
///
/// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
pub async fn delete(engine: &EngineHandle, actor_id: Uuid, run_id: Uuid) -> Result<()> {
    let registry = engine.registry();
    // Surface a clean NotFound when the run doesn't exist instead
    // of silently succeeding.
    let _header = registry.get_run(actor_id, run_id).await?;
    registry.delete_run_bodies(actor_id, run_id).await?;
    registry.delete_run(actor_id, run_id).await
}

/// Resolve every `(policy_id, version)` ref in the run header to
/// its persisted [`Policy`] blob. Missing refs fail the apply
/// call (the per-doc filter operates on the full set; a missing
/// policy can't be silently dropped).
async fn resolve_policies(
    registry: &crate::registry::RegistryHandle,
    actor_id: Uuid,
    refs: &[super::state::ResourceRef],
) -> Result<Vec<nvisy_core::policy::Policy>> {
    let mut out = Vec::with_capacity(refs.len());
    for r in refs {
        let policy = registry
            .get_policy(actor_id, r.id, r.version.clone())
            .await?;
        out.push(policy);
    }
    Ok(out)
}

/// One per-doc unit of the apply fan-out. Returns `true` on
/// success, `false` when any step failed (failures land in the
/// row's state; the boolean only drives the header transition).
///
/// Inputs are owned (analyzer spec, metadata, Arc-shared
/// policy set) so the resulting future captures no references
/// on its caller's stack — keeps the outer `runs::apply`
/// future composable with handler-level `Send + 'static`
/// bounds.
async fn apply_one_doc(
    engine: &EngineHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
    request_metadata: std::collections::HashMap<String, String>,
    spec: nvisy_core::plan::AnalyzerSpec,
    policies: std::sync::Arc<Vec<nvisy_core::policy::Policy>>,
) -> bool {
    let registry = engine.registry();
    let Ok(mut doc) = registry.get_run_doc(actor_id, run_id, doc_id).await else {
        return false;
    };
    // Only apply rows that finished analyze. Failed / TimedOut /
    // already-Applied rows pass through untouched.
    if !matches!(doc.state, RunDocState::AwaitingReview) {
        return matches!(doc.state, RunDocState::Applied);
    }

    let (input_file, input_bytes) = match load_input(registry, actor_id, doc.input_file_id).await {
        Ok(loaded) => loaded,
        Err(err) => {
            doc.state = RunDocState::Failed {
                reason: format!("input file unavailable: {err}"),
            };
            let _ = registry.put_run_doc(actor_id, run_id, &doc).await;
            return false;
        }
    };

    let merged = merge_metadata(&input_file.descriptor_metadata, &request_metadata);
    let facts = DocumentFacts {
        labels: &input_file.descriptor_labels,
        metadata: &merged,
    };

    let outcome = apply_document(
        engine.formats(),
        input_bytes,
        input_file.extension.as_str(),
        &spec,
        policies.as_slice(),
        &facts,
        &doc.body,
    )
    .await;

    match outcome {
        Ok(out) => {
            let descriptor = redacted_descriptor(&input_file, run_id);
            match registry.put_file(actor_id, descriptor, out.bytes).await {
                Ok(output_file) => {
                    doc.output_file_id = Some(output_file.id);
                    doc.state = RunDocState::Applied;
                    let _ = registry.put_run_doc(actor_id, run_id, &doc).await;
                    true
                }
                Err(err) => {
                    doc.state = RunDocState::Failed {
                        reason: format!("writing redacted output file failed: {err}"),
                    };
                    let _ = registry.put_run_doc(actor_id, run_id, &doc).await;
                    false
                }
            }
        }
        Err(err) => {
            doc.state = RunDocState::Failed {
                reason: err.to_string(),
            };
            let _ = registry.put_run_doc(actor_id, run_id, &doc).await;
            false
        }
    }
}

/// Build the descriptor for a redacted output file. Inherits
/// extension + content_type + descriptor labels/metadata from
/// the input file, appends `-redacted` to the filename, stamps
/// the lineage so audits + clients can trace the output back to
/// its source and the run that produced it.
fn redacted_descriptor(input: &FileMetadata, run_id: Uuid) -> FileDescriptor {
    let filename = input
        .filename
        .as_ref()
        .map(|name| redacted_filename(name.as_str()).into());
    FileDescriptor {
        filename,
        content_type: input.content_type.clone(),
        extension: input.extension.clone(),
        lineage: Some(FileLineage::RedactedFrom {
            run_id,
            source_file_id: input.id,
        }),
        descriptor_labels: input.descriptor_labels.clone(),
        descriptor_metadata: input.descriptor_metadata.clone(),
    }
}

/// Insert `-redacted` before the file's final extension —
/// `report.pdf` → `report-redacted.pdf`, `archive.tar.gz` →
/// `archive.tar-redacted.gz`. Pure naming convenience for the
/// download UX; identity is the UUID.
fn redacted_filename(name: &str) -> String {
    match name.rfind('.') {
        Some(idx) if idx > 0 => format!("{}-redacted{}", &name[..idx], &name[idx..]),
        _ => format!("{name}-redacted"),
    }
}

/// Apply a reviewer override to one entity.
///
/// Loads the doc body, finds the entity by id, sets the override,
/// writes the body back. Idempotent — overriding the same entity
/// twice is well-defined (second write wins).
///
/// Returns [`ErrorKind::NotFound`] when the run, doc, or entity
/// id is unknown.
///
/// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
pub async fn override_entity(
    engine: &EngineHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
    entity_id: Uuid,
    action: nvisy_core::policy::RuleAction,
) -> Result<()> {
    let registry = engine.registry();
    let mut doc = registry.get_run_doc(actor_id, run_id, doc_id).await?;
    let found = patch_override(&mut doc.body, entity_id, action);
    if !found {
        return Err(nvisy_core::Error::not_found(
            format!("entity {entity_id} not found in run {run_id} doc {doc_id}"),
            "runs::override_entity",
        ));
    }
    registry.put_run_doc(actor_id, run_id, &doc).await
}

fn patch_override(
    body: &mut DocBody,
    entity_id: Uuid,
    action: nvisy_core::policy::RuleAction,
) -> bool {
    match body {
        DocBody::Text { entities } => patch_each(entities, entity_id, action),
        DocBody::Tabular { entities } => patch_each(entities, entity_id, action),
        DocBody::Image { entities } => patch_each(entities, entity_id, action),
        DocBody::Audio { entities } => patch_each(entities, entity_id, action),
    }
}

fn patch_each<M: elide_core::modality::Modality>(
    entities: &mut [super::state::EntityRecord<M>],
    entity_id: Uuid,
    action: nvisy_core::policy::RuleAction,
) -> bool {
    for record in entities.iter_mut() {
        if record.entity.id == entity_id {
            record.r#override = Some(action);
            return true;
        }
    }
    false
}

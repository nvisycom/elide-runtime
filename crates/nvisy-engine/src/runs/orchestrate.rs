//! Run lifecycle orchestration: [`start`] persists inputs +
//! Queued per-doc rows, fans the analyzer out across documents,
//! and transitions the run to [`RunState::AwaitingReview`].
//! [`apply`] is still a scaffold (per-doc apply lands in E3.I).
//!
//! Fan-out shape:
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
//! deliberate: each per-doc future borrows the codec registry +
//! analyzer spec, and `JoinSet::spawn` would force them to
//! `'static`. The stream combinator keeps the borrows live for
//! free.

use std::time::Duration;

use elide::codec::FormatRegistry;
use futures::StreamExt;
use jiff::Timestamp;
use nvisy_core::Result;
use uuid::Uuid;

use crate::registry::RegistryHandle;

use super::filter::{DocumentFacts, merge_metadata};
use super::input::StartBatch;
use super::persist::{
    get_doc, get_header, get_input, put_artifact, put_doc, put_header, put_input,
};
use super::pipeline::{analyze_document, apply_document};
use super::state::{
    DocBody, ModalityKind, Run, RunDocState, RunDocument, RunState,
};

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
pub async fn start(
    handle: &RegistryHandle,
    formats: &FormatRegistry,
    actor_id: Uuid,
    batch: StartBatch,
) -> Result<Uuid> {
    let run_id = Uuid::now_v7();
    let now = Timestamp::now();
    let concurrency = batch.concurrency.unwrap_or(DEFAULT_CONCURRENCY).max(1);

    // Persist input bytes + a Queued per-doc row for every input
    // up-front so the run is fully queryable from the moment the
    // header lands. `modality` defaults to Text and gets corrected
    // once the codec resolves; `body` stays empty until analyze
    // finishes.
    let mut document_ids = Vec::with_capacity(batch.documents.len());
    for doc in &batch.documents {
        let doc_id = Uuid::now_v7();
        document_ids.push(doc_id);

        put_input(handle, actor_id, run_id, doc_id, doc.bytes.clone()).await?;

        let modality = ModalityKind::Text;
        let document = RunDocument {
            id: doc_id,
            extension: doc.extension.clone(),
            descriptor_labels: doc.descriptor_labels.clone(),
            descriptor_metadata: doc.descriptor_metadata.clone(),
            state: RunDocState::Queued,
            modality,
            body: DocBody::empty(modality),
            has_artifact: false,
        };
        put_doc(handle, actor_id, run_id, &document).await?;
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
    put_header(handle, actor_id, &run).await?;

    fanout_analyze(
        handle,
        formats,
        actor_id,
        run_id,
        &document_ids,
        &batch.documents,
        &run.analyzer,
        concurrency,
    )
    .await?;

    // All per-doc rows have settled in their terminal analyze
    // state (AwaitingReview / Failed / TimedOut); flip the header
    // so callers see the run is ready for review.
    let mut header = run;
    header.state = RunState::AwaitingReview;
    header.updated_at = Timestamp::now();
    put_header(handle, actor_id, &header).await?;

    Ok(run_id)
}

/// Drive the per-doc analyze fan-out as a bounded-concurrency
/// stream.
///
/// One per-doc future per `(doc_id, document)` pair; the stream
/// caps in-flight work at `concurrency`. Each future:
///   1. flips the per-doc state to Analyzing,
///   2. calls the pipeline under a hard timeout,
///   3. writes the per-doc row with the resolved state.
///
/// Per-doc failures are recorded in fjall, not returned — the
/// outer Result only fails on infrastructure-level errors.
async fn fanout_analyze(
    handle: &RegistryHandle,
    formats: &FormatRegistry,
    actor_id: Uuid,
    run_id: Uuid,
    document_ids: &[Uuid],
    documents: &[super::input::DocumentInput],
    spec: &nvisy_core::plan::AnalyzerSpec,
    concurrency: usize,
) -> Result<()> {
    let work = document_ids.iter().zip(documents.iter()).map(|(doc_id, doc)| {
        analyze_one_doc(handle, formats, actor_id, run_id, *doc_id, doc, spec)
    });

    futures::stream::iter(work)
        .buffer_unordered(concurrency)
        .for_each(|()| async {})
        .await;

    Ok(())
}

/// One per-doc unit of the analyze fan-out. Owns the lifecycle
/// transitions for one row; never returns an error (failures
/// land in the row's state).
async fn analyze_one_doc(
    handle: &RegistryHandle,
    formats: &FormatRegistry,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
    doc: &super::input::DocumentInput,
    spec: &nvisy_core::plan::AnalyzerSpec,
) {
    mark_analyzing(handle, actor_id, run_id, doc_id).await;

    let analyze = analyze_document(formats, doc.bytes.clone(), &doc.extension, spec);
    let outcome = match tokio::time::timeout(PER_DOC_TIMEOUT, analyze).await {
        Ok(Ok(outcome)) => Ok(outcome),
        Ok(Err(err)) => Err(DocFailure::Failed(err.to_string())),
        Err(_) => Err(DocFailure::TimedOut),
    };

    write_outcome(handle, actor_id, run_id, doc_id, outcome).await;
}

enum DocFailure {
    Failed(String),
    TimedOut,
}

async fn mark_analyzing(
    handle: &RegistryHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
) {
    let Ok(mut doc) = get_doc(handle, actor_id, run_id, doc_id).await else { return };
    doc.state = RunDocState::Analyzing;
    let _ = put_doc(handle, actor_id, run_id, &doc).await;
}

async fn write_outcome(
    handle: &RegistryHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
    outcome: std::result::Result<super::pipeline::AnalyzeOutcome, DocFailure>,
) {
    let Ok(mut doc) = get_doc(handle, actor_id, run_id, doc_id).await else { return };
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
    let _ = put_doc(handle, actor_id, run_id, &doc).await;
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
pub async fn apply(
    handle: &RegistryHandle,
    formats: &FormatRegistry,
    actor_id: Uuid,
    run_id: Uuid,
) -> Result<()> {
    let mut run = get_header(handle, actor_id, run_id).await?;
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
    let policies = resolve_policies(handle, actor_id, &run.policy_refs).await?;

    let outcomes = fanout_apply(
        handle,
        formats,
        actor_id,
        run_id,
        &run.document_ids,
        &run.metadata,
        &run.analyzer,
        &policies,
        run.concurrency,
    )
    .await;

    let all_applied = outcomes.iter().all(|ok| *ok);
    run.state = if all_applied {
        RunState::Applied
    } else {
        RunState::PartiallyApplied
    };
    run.updated_at = Timestamp::now();
    put_header(handle, actor_id, &run).await?;
    Ok(())
}

/// Resolve every `(policy_id, version)` ref in the run header to
/// its persisted [`Policy`] blob. Missing refs fail the apply
/// call (the per-doc filter operates on the full set; a missing
/// policy can't be silently dropped).
async fn resolve_policies(
    handle: &RegistryHandle,
    actor_id: Uuid,
    refs: &[super::state::ResourceRef],
) -> Result<Vec<nvisy_core::policy::Policy>> {
    let mut out = Vec::with_capacity(refs.len());
    for r in refs {
        let policy = crate::policies::get(handle, actor_id, r.id, r.version.clone()).await?;
        out.push(policy);
    }
    Ok(out)
}

async fn fanout_apply(
    handle: &RegistryHandle,
    formats: &FormatRegistry,
    actor_id: Uuid,
    run_id: Uuid,
    document_ids: &[Uuid],
    request_metadata: &std::collections::HashMap<String, String>,
    spec: &nvisy_core::plan::AnalyzerSpec,
    policies: &[nvisy_core::policy::Policy],
    concurrency: usize,
) -> Vec<bool> {
    let work = document_ids.iter().map(|doc_id| {
        apply_one_doc(
            handle,
            formats,
            actor_id,
            run_id,
            *doc_id,
            request_metadata,
            spec,
            policies,
        )
    });

    futures::stream::iter(work)
        .buffer_unordered(concurrency)
        .collect::<Vec<bool>>()
        .await
}

/// One per-doc unit of the apply fan-out. Returns `true` on
/// success, `false` when any step failed (failures land in the
/// row's state; the boolean only drives the header transition).
async fn apply_one_doc(
    handle: &RegistryHandle,
    formats: &FormatRegistry,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
    request_metadata: &std::collections::HashMap<String, String>,
    spec: &nvisy_core::plan::AnalyzerSpec,
    policies: &[nvisy_core::policy::Policy],
) -> bool {
    let Ok(mut doc) = get_doc(handle, actor_id, run_id, doc_id).await else {
        return false;
    };
    // Only apply rows that finished analyze. Failed / TimedOut /
    // already-Applied rows pass through untouched.
    if !matches!(doc.state, RunDocState::AwaitingReview) {
        return matches!(doc.state, RunDocState::Applied);
    }

    let Ok(bytes) = get_input(handle, actor_id, run_id, doc_id).await else {
        doc.state = RunDocState::Failed {
            reason: "input bytes missing from run_inputs keyspace".into(),
        };
        let _ = put_doc(handle, actor_id, run_id, &doc).await;
        return false;
    };

    let merged = merge_metadata(&doc.descriptor_metadata, request_metadata);
    let facts = DocumentFacts {
        labels: &doc.descriptor_labels,
        metadata: &merged,
    };

    let outcome =
        apply_document(formats, bytes, &doc.extension, spec, policies, &facts, &doc.body).await;

    match outcome {
        Ok(out) => {
            if put_artifact(handle, actor_id, run_id, doc_id, out.bytes).await.is_err() {
                doc.state = RunDocState::Failed {
                    reason: "writing redacted artifact failed".into(),
                };
                let _ = put_doc(handle, actor_id, run_id, &doc).await;
                return false;
            }
            doc.has_artifact = true;
            doc.state = RunDocState::Applied;
            let _ = put_doc(handle, actor_id, run_id, &doc).await;
            true
        }
        Err(err) => {
            doc.state = RunDocState::Failed { reason: err.to_string() };
            let _ = put_doc(handle, actor_id, run_id, &doc).await;
            false
        }
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
    handle: &RegistryHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
    entity_id: Uuid,
    action: nvisy_core::policy::RuleAction,
) -> Result<()> {
    let mut doc = super::persist::get_doc(handle, actor_id, run_id, doc_id).await?;
    let found = patch_override(&mut doc.body, entity_id, action);
    if !found {
        return Err(nvisy_core::Error::not_found(
            format!("entity {entity_id} not found in run {run_id} doc {doc_id}"),
            "runs::override_entity",
        ));
    }
    super::persist::put_doc(handle, actor_id, run_id, &doc).await
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

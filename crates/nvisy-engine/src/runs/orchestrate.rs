//! Run lifecycle: analyze fan-out, per-doc apply, and the state
//! transitions between them.

use std::collections::HashMap;
use std::result::Result as StdResult;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use elide_core::modality::Modality;
use futures::{StreamExt, stream};
use jiff::Timestamp;
use nvisy_core::file::FileMetadata;
use nvisy_core::plan::AnalyzerParams;
use nvisy_core::policy::{Policy, Retention, RetentionScope, RuleAction, resolve_retention};
use nvisy_core::{Error, FileLineage, RawDocument, Result};
use tokio::time::timeout;
use uuid::Uuid;

use super::filter::{DocumentFacts, merge_metadata, policy_applies};
use super::input::StartBatch;
use super::persist::RunRegistry;
use super::state::{
    DocBody, EntityRecord, RecognizedGroup, ResourceRef, Run, RunDocState, RunDocument, RunState,
};
use crate::keyspace::FileDescriptor;
use crate::registry::RegistryHandle;
use crate::retention::active_refs::ActiveFileRefRegistry;
use crate::retention::schedule::{RetentionRecord, RetentionRegistry};
use crate::{Engine, FileRegistry, PolicyRegistry};

/// Default per-run concurrency cap when the caller's
/// [`StartBatch::concurrency`] is `None`.
const DEFAULT_CONCURRENCY: usize = 4;

/// Per-doc analyze hard timeout. Recognizers that exceed this
/// land in [`RunDocState::TimedOut`]; the rest of the run
/// continues.
const PER_DOC_TIMEOUT: Duration = Duration::from_secs(120);

impl Engine {
    /// Start a run.
    ///
    /// Mints a UUIDv7 run id and per-doc ids, persists a Queued
    /// per-doc row for every input, writes the run header in
    /// [`RunState::Analyzing`], then fans the analyzer out across
    /// docs. Each task rewrites its per-doc row with the
    /// recognized entities and new state. When the fan-out
    /// finishes, the header flips to
    /// [`RunState::AwaitingReview`].
    ///
    /// Returns the new run id; the per-doc results are queryable
    /// via [`Engine::get_run_doc`].
    pub async fn start_run(&self, actor_id: Uuid, batch: StartBatch) -> Result<Uuid> {
        let run_id = Uuid::now_v7();
        let now = Timestamp::now();
        let concurrency = batch.concurrency.unwrap_or(DEFAULT_CONCURRENCY).max(1);
        let registry = self.registry();

        // Load every referenced policy up-front so we can pin the
        // resolved retention rules onto each input file before
        // analyze fans out. A caller who submits ZeroRetention
        // expects the sweeper to see the row from the moment the
        // run starts, not just after apply.
        let policies = resolve_policies(registry, actor_id, &batch.policy_refs).await?;
        let retention = resolve_retention(&policies);

        let mut document_ids = Vec::with_capacity(batch.documents.len());
        for doc in &batch.documents {
            let doc_id = Uuid::now_v7();
            document_ids.push(doc_id);

            let document = RunDocument {
                id: doc_id,
                input_file_id: doc.file_id,
                output_file_id: None,
                state: RunDocState::Queued,
                body: DocBody::default(),
            };
            registry.put_run_doc(actor_id, run_id, &document).await?;

            // Reverse-index the input for the sweeper's active-run
            // gate: as long as this run's ref row exists, the
            // sweeper defers on this file.
            registry
                .insert_active_ref(actor_id, doc.file_id, run_id)
                .await?;

            // Pin OriginalContent retention on the input file.
            // No-op when the scope resolves to Indefinite or the
            // policy set is silent on it.
            registry
                .pin_retention(
                    actor_id,
                    doc.file_id,
                    RetentionScope::OriginalContent,
                    &retention,
                    run_id,
                    now,
                )
                .await?;
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

        let analyze_inputs: Vec<(Uuid, Uuid)> = document_ids
            .iter()
            .zip(batch.documents.iter())
            .map(|(doc_id, doc)| (*doc_id, doc.file_id))
            .collect();
        let analyzer_spec = Arc::new(run.analyzer.clone());
        let work = analyze_inputs.into_iter().map(|(doc_id, file_id)| {
            self.analyze_one_doc(actor_id, run_id, doc_id, file_id, Arc::clone(&analyzer_spec))
        });
        stream::iter(work)
            .buffer_unordered(concurrency)
            .for_each(|()| async {})
            .await;

        let mut header = run;
        header.state = RunState::AwaitingReview;
        header.updated_at = Timestamp::now();
        registry.put_run(actor_id, &header).await?;

        Ok(run_id)
    }

    /// Apply a run.
    ///
    /// Loads the run header, verifies it is in
    /// [`RunState::AwaitingReview`], resolves every referenced
    /// policy, fans the per-doc anonymizer pass out under the
    /// run's concurrency cap, persists the redacted bytes as
    /// artifacts, and rewrites the header to
    /// [`RunState::Applied`] (every doc applied) or
    /// [`RunState::PartiallyApplied`] (at least one failed).
    pub async fn apply_run(&self, actor_id: Uuid, run_id: Uuid) -> Result<()> {
        let registry = self.registry();
        let mut run = registry.get_run(actor_id, run_id).await?;
        if !matches!(run.state, RunState::AwaitingReview) {
            return Err(Error::conflict(
                format!(
                    "run {run_id} is in state {:?}; apply only valid from AwaitingReview",
                    run.state,
                ),
                "Engine::apply_run",
            ));
        }

        // Retention is re-resolved here (start also resolved it,
        // for the input pin); the resolution is deterministic on
        // the same policy set, so both callers converge on the
        // same value.
        let policies = resolve_policies(registry, actor_id, &run.policy_refs).await?;
        let retention = resolve_retention(policies.iter());

        let doc_ids: Vec<Uuid> = run.document_ids.clone();
        let concurrency = run.concurrency;
        let ctx = Arc::new(ApplyContext {
            request_metadata: run.metadata.clone(),
            spec: run.analyzer.clone(),
            policies,
            retention,
        });
        let work = doc_ids.iter().copied().map(|doc_id| {
            self.apply_one_doc(actor_id, run_id, doc_id, Arc::clone(&ctx))
        });
        let outcomes: Vec<bool> = stream::iter(work)
            .buffer_unordered(concurrency)
            .collect()
            .await;

        self.release_active_refs(actor_id, run_id, &doc_ids).await?;

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

    /// Read the run header at `(actor_id, run_id)`.
    pub async fn get_run(&self, actor_id: Uuid, run_id: Uuid) -> Result<Run> {
        self.registry().get_run(actor_id, run_id).await
    }

    /// Read a per-doc body at `(actor_id, run_id, doc_id)`.
    pub async fn get_run_doc(
        &self,
        actor_id: Uuid,
        run_id: Uuid,
        doc_id: Uuid,
    ) -> Result<RunDocument> {
        self.registry()
            .get_run_doc(actor_id, run_id, doc_id)
            .await
    }

    /// List every retention schedule row for a single file
    /// (`(actor_id, file_id)`). Returns one entry per scope that
    /// has an expiring rule (`Zero` / `Duration`); scopes with
    /// `Indefinite` or no rule are absent (they have no row).
    pub async fn list_retention_for_file(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
    ) -> Result<Vec<RetentionRecord>> {
        self.registry()
            .list_retention_for_file(actor_id, file_id)
            .await
    }

    /// Fetch one file's retention row for a specific scope, or
    /// `None` when no row exists (the absence encodes
    /// "indefinite" or "not yet scheduled").
    pub async fn find_retention(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
        scope: nvisy_core::policy::RetentionScope,
    ) -> Result<Option<RetentionRecord>> {
        self.registry()
            .find_retention(actor_id, file_id, scope)
            .await
    }

    /// Whether any active (non-terminal) run currently references
    /// `(actor_id, file_id)`. The sweeper's gate; also the
    /// integration-test read entry for asserting run-lifecycle
    /// wiring.
    pub async fn has_active_refs(&self, actor_id: Uuid, file_id: Uuid) -> Result<bool> {
        self.registry().has_active_refs(actor_id, file_id).await
    }

    /// Drop every active-file-reference row belonging to
    /// `run_id`. Harvests input file ids off the per-doc rows
    /// (the source of truth) so a partially-failed start —
    /// `put_run_doc` succeeded but `insert_active_ref` didn't —
    /// still gets fully cleared. Per-doc reads that fail (e.g.
    /// the row was manually deleted between start and now) are
    /// skipped: the corresponding active-ref, if it exists,
    /// will get picked up by the startup reap.
    ///
    /// Called at every terminal transition
    /// ([`Engine::apply_run`], [`Engine::cancel_run`],
    /// [`Engine::delete_run`]).
    async fn release_active_refs(
        &self,
        actor_id: Uuid,
        run_id: Uuid,
        doc_ids: &[Uuid],
    ) -> Result<()> {
        let registry = self.registry();
        let mut input_file_ids: Vec<Uuid> = Vec::with_capacity(doc_ids.len());
        for &doc_id in doc_ids {
            if let Ok(doc) = registry.get_run_doc(actor_id, run_id, doc_id).await {
                input_file_ids.push(doc.input_file_id);
            }
        }
        registry
            .delete_active_refs_for_run(actor_id, &input_file_ids, run_id)
            .await
    }

    /// List every run for `actor_id`. Returns full headers;
    /// callers filter by [`RunState`].
    pub async fn list_runs(&self, actor_id: Uuid) -> Vec<Run> {
        self.registry()
            .list_runs(actor_id)
            .await
            .unwrap_or_default()
    }

    /// Cancel a run.
    ///
    /// Marks the header [`RunState::Failed`] with
    /// `reason = "cancelled"`. Cancel is only valid from
    /// [`RunState::Analyzing`] or [`RunState::AwaitingReview`];
    /// calling it on a terminal run ([`RunState::Applied`] /
    /// [`RunState::PartiallyApplied`] / [`RunState::Failed`])
    /// returns [`ErrorKind::Conflict`].
    ///
    /// Cancel is a *header* operation: in-flight per-doc fan-out
    /// tasks are not interrupted; they finish their current step
    /// and write their outcome into a doc row no one will read.
    /// Cooperative interruption needs a
    /// [`tokio_util::sync::CancellationToken`] threaded through
    /// the pipeline.
    ///
    /// [`ErrorKind::Conflict`]: nvisy_core::ErrorKind::Conflict
    /// [`tokio_util::sync::CancellationToken`]: https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html
    pub async fn cancel_run(&self, actor_id: Uuid, run_id: Uuid) -> Result<()> {
        let registry = self.registry();
        let mut run = registry.get_run(actor_id, run_id).await?;
        match run.state {
            RunState::Analyzing | RunState::AwaitingReview => {}
            ref other => {
                return Err(Error::conflict(
                    format!(
                        "run {run_id} is in state {other:?}; cancel only valid from Analyzing or AwaitingReview"
                    ),
                    "Engine::cancel_run",
                ));
            }
        }

        self.release_active_refs(actor_id, run_id, &run.document_ids)
            .await?;

        run.state = RunState::Failed {
            reason: "cancelled".to_owned(),
        };
        run.updated_at = Timestamp::now();
        registry.put_run(actor_id, &run).await
    }

    /// Delete a run, cascading across every run keyspace.
    ///
    /// Removes the header (`run_headers`) plus every per-doc
    /// body (`run_docs`) belonging to the run. Returns
    /// [`ErrorKind::NotFound`] when the header is missing.
    ///
    /// In-flight cancellation is **not** part of delete; callers
    /// should [`Engine::cancel_run`] first when the run is still
    /// active, then delete. Calling `delete_run` on an active
    /// run is allowed — per-doc tasks that complete after the
    /// delete write into keyspaces that no longer have a header,
    /// and are reaped on the next list scan.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    pub async fn delete_run(&self, actor_id: Uuid, run_id: Uuid) -> Result<()> {
        let registry = self.registry();
        let header = registry.get_run(actor_id, run_id).await?;

        // Release refs before dropping the per-doc bodies — the
        // docs are the only source of truth for input_file_id
        // and go away next.
        self.release_active_refs(actor_id, run_id, &header.document_ids)
            .await?;

        registry.delete_run_bodies(actor_id, run_id).await?;
        registry.delete_run(actor_id, run_id).await
    }

    /// Apply a reviewer override to one entity.
    ///
    /// Loads the doc body, finds the entity by id (walking the
    /// body group and every container part), sets the override,
    /// writes the body back. Idempotent — overriding the same
    /// entity twice is well-defined (second write wins).
    ///
    /// Returns [`ErrorKind::NotFound`] when the run, doc, or
    /// entity id is unknown.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    pub async fn override_entity(
        &self,
        actor_id: Uuid,
        run_id: Uuid,
        doc_id: Uuid,
        entity_id: Uuid,
        action: RuleAction,
    ) -> Result<()> {
        let registry = self.registry();
        let mut doc = registry.get_run_doc(actor_id, run_id, doc_id).await?;
        let found = patch_override(&mut doc.body, entity_id, action);
        if !found {
            return Err(Error::not_found(
                format!("entity {entity_id} not found in run {run_id} doc {doc_id}"),
                "Engine::override_entity",
            ));
        }
        registry.put_run_doc(actor_id, run_id, &doc).await
    }

    /// One per-doc unit of the analyze fan-out. Owns the
    /// lifecycle transitions for one row; never returns an error
    /// (failures land in the row's state).
    async fn analyze_one_doc(
        &self,
        actor_id: Uuid,
        run_id: Uuid,
        doc_id: Uuid,
        input_file_id: Uuid,
        spec: Arc<AnalyzerParams>,
    ) {
        let registry = self.registry();
        mark_analyzing(registry, actor_id, run_id, doc_id).await;

        let outcome = match load_input(registry, actor_id, input_file_id).await {
            Ok((file, bytes)) => {
                let document = RawDocument {
                    bytes,
                    extension: file.extension,
                    content_type: file.content_type,
                };
                let analyze = self.analyze_document(document, &spec, run_id);
                match timeout(PER_DOC_TIMEOUT, analyze).await {
                    Ok(Ok(outcome)) => Ok(outcome),
                    Ok(Err(err)) => Err(DocFailure::Failed(err.to_string())),
                    Err(_) => Err(DocFailure::TimedOut),
                }
            }
            Err(err) => Err(DocFailure::Failed(err.to_string())),
        };

        write_outcome(registry, actor_id, run_id, doc_id, outcome).await;
    }

    /// One per-doc unit of the apply fan-out. Returns `true` on
    /// success, `false` when any step failed (failures land in
    /// the row's state; the boolean only drives the header
    /// transition).
    async fn apply_one_doc(
        &self,
        actor_id: Uuid,
        run_id: Uuid,
        doc_id: Uuid,
        ctx: Arc<ApplyContext>,
    ) -> bool {
        let registry = self.registry();
        let Ok(mut doc) = registry.get_run_doc(actor_id, run_id, doc_id).await else {
            return false;
        };
        if !matches!(doc.state, RunDocState::AwaitingReview) {
            return matches!(doc.state, RunDocState::Applied);
        }

        let (input_file, input_bytes) =
            match load_input(registry, actor_id, doc.input_file_id).await {
                Ok(loaded) => loaded,
                Err(err) => {
                    doc.state = RunDocState::Failed {
                        reason: format!("input file unavailable: {err}"),
                    };
                    let _ = registry.put_run_doc(actor_id, run_id, &doc).await;
                    return false;
                }
            };

        let merged = merge_metadata(&input_file.descriptor_metadata, &ctx.request_metadata);
        let facts = DocumentFacts {
            labels: &input_file.descriptor_labels,
            metadata: &merged,
        };
        let scoped: Vec<Policy> = ctx
            .policies
            .iter()
            .filter(|p| policy_applies(p, &facts))
            .cloned()
            .collect();

        let document = RawDocument {
            bytes: input_bytes,
            extension: input_file.extension.clone(),
            content_type: input_file.content_type.clone(),
        };
        let outcome = self
            .apply_document(document, &ctx.spec, &scoped, &doc.body, run_id)
            .await;

        match outcome {
            Ok(out) => {
                let descriptor = redacted_descriptor(&input_file, run_id);
                match registry.put_file(actor_id, descriptor, out.bytes).await {
                    Ok(output_file) => {
                        doc.output_file_id = Some(output_file.id);
                        doc.state = RunDocState::Applied;
                        let _ = registry.put_run_doc(actor_id, run_id, &doc).await;
                        // Pin RedactedOutput retention on the
                        // freshly stored output. Best-effort: a
                        // write failure here does not roll back
                        // the apply — the redaction is done, the
                        // row is missing, the artifact just
                        // won't be swept.
                        if let Err(err) = registry
                            .pin_retention(
                                actor_id,
                                output_file.id,
                                RetentionScope::RedactedOutput,
                                &ctx.retention,
                                run_id,
                                Timestamp::now(),
                            )
                            .await
                        {
                            tracing::warn!(
                                target: "engine::apply",
                                actor_id = %actor_id,
                                run_id = %run_id,
                                output_file_id = %output_file.id,
                                error = %err,
                                "failed to pin RedactedOutput retention row",
                            );
                        }
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
}

enum DocFailure {
    Failed(String),
    TimedOut,
}

/// Per-run inputs shared across every doc in the apply
/// fan-out. Materialised once by `apply_run` and passed by
/// `Arc` to each per-doc task so the fan-out captures no
/// borrows on its caller's stack.
struct ApplyContext {
    /// Per-request metadata; merged with each doc's descriptor
    /// metadata at `Policy::applies_when` evaluation time.
    request_metadata: HashMap<String, String>,
    /// The recognition plan the run was started with. Threaded
    /// into `Engine::apply_document` for the label catalog.
    spec: AnalyzerParams,
    /// Full resolved policy set. Each per-doc task filters this
    /// against its own `DocumentFacts`.
    policies: Vec<Policy>,
    /// Strictest resolved retention per scope, from
    /// [`resolve_retention`]. The apply path reads
    /// [`RetentionScope::RedactedOutput`] out of this for each
    /// output file; scopes absent from the map (no policy
    /// governs) or resolving to `Indefinite` are no-ops.
    ///
    /// [`resolve_retention`]: nvisy_core::policy::resolve_retention
    retention: HashMap<RetentionScope, Retention>,
}

/// Fetch the file metadata + bytes for one input file. Two
/// reads (metadata then content) because the file API splits
/// them across keyspaces.
async fn load_input(
    registry: &RegistryHandle,
    actor_id: Uuid,
    file_id: Uuid,
) -> Result<(FileMetadata, Bytes)> {
    let file = registry.get_file(actor_id, file_id).await?;
    let bytes = registry.get_file_bytes(actor_id, file_id).await?;
    Ok((file, bytes))
}

async fn mark_analyzing(registry: &RegistryHandle, actor_id: Uuid, run_id: Uuid, doc_id: Uuid) {
    let Ok(mut doc) = registry.get_run_doc(actor_id, run_id, doc_id).await else {
        return;
    };
    doc.state = RunDocState::Analyzing;
    let _ = registry.put_run_doc(actor_id, run_id, &doc).await;
}

async fn write_outcome(
    registry: &RegistryHandle,
    actor_id: Uuid,
    run_id: Uuid,
    doc_id: Uuid,
    outcome: StdResult<DocBody, DocFailure>,
) {
    let Ok(mut doc) = registry.get_run_doc(actor_id, run_id, doc_id).await else {
        return;
    };
    match outcome {
        Ok(body) => {
            doc.body = body;
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

/// Resolve every `(policy_id, version)` ref to its persisted
/// [`Policy`] blob. Missing refs fail the whole call — the
/// per-doc filter operates on the full set; a missing policy
/// can't be silently dropped.
async fn resolve_policies(
    registry: &RegistryHandle,
    actor_id: Uuid,
    refs: &[ResourceRef],
) -> Result<Vec<Policy>> {
    let mut out = Vec::with_capacity(refs.len());
    for r in refs {
        let policy = registry
            .get_policy(actor_id, r.id, r.version.clone())
            .await?;
        out.push(policy);
    }
    Ok(out)
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

fn patch_override(doc_body: &mut DocBody, entity_id: Uuid, action: RuleAction) -> bool {
    // Walk body then every part; at most one slot matches since
    // entity ids are globally unique.
    let slot = doc_body
        .body
        .as_mut()
        .and_then(|g| find_override_slot(g, entity_id))
        .or_else(|| {
            doc_body
                .parts
                .values_mut()
                .find_map(|g| find_override_slot(g, entity_id))
        });
    match slot {
        Some(slot) => {
            *slot = Some(action);
            true
        }
        None => false,
    }
}

fn find_override_slot(
    group: &mut RecognizedGroup,
    entity_id: Uuid,
) -> Option<&mut Option<RuleAction>> {
    match group {
        RecognizedGroup::Text { entities } => find_in(entities, entity_id),
        RecognizedGroup::Tabular { entities } => find_in(entities, entity_id),
        RecognizedGroup::Image { entities } => find_in(entities, entity_id),
        RecognizedGroup::Audio { entities } => find_in(entities, entity_id),
    }
}

fn find_in<M: Modality>(
    entities: &mut [EntityRecord<M>],
    entity_id: Uuid,
) -> Option<&mut Option<RuleAction>> {
    entities
        .iter_mut()
        .find(|r| r.entity.id == entity_id)
        .map(|r| &mut r.r#override)
}

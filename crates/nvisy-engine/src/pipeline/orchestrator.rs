//! Pipeline orchestrator: concurrent document processing through a
//! typed execution plan.
//!
//! The [`Orchestrator`] drives the pipeline at the top level: it
//! imports documents, fans them out to concurrent [`DocumentPipeline`]
//! tasks (one per document), and collects the results.
//!
//! [`DocumentPipeline`] processes a single document through all plan
//! phases sequentially: extraction → detection → deduplication → redaction →
//! validation.

use std::future::Future;
use std::sync::Arc;

use nvisy_core::Error;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use super::plan::{ExecutionPlan, ExportStep, ImportStep, PhasePolicy};
use crate::detection::DetectionEngine;
use crate::extraction::Extractors;
use crate::graph::TimeoutExt;
use crate::operation::{
    Deduplication, Detection, DocumentEnvelope, ExportFile, GenerateContext, ImportFile, Operation,
    SharedData, Validation,
};
use crate::redaction::{Redaction, RedactorDefaults};
use crate::workflow::{
    ConcurrencyPolicy, Detection as DetectionConfig, Extraction as ExtractionConfig,
};

const TARGET: &str = "nvisy_engine::pipeline::orchestrator";

/// Per-run execution context shared across all document tasks.
pub(super) struct RunContext {
    /// Token to signal cancellation to all tasks.
    pub cancel: CancellationToken,
    /// Shared run-wide state: run ID, actor, registry, policies.
    pub shared: Arc<SharedData>,
    /// Pre-built extractor registry from `RuntimeConfig.extractor`.
    /// Shared across every run.
    pub extractors: Arc<Extractors>,
    /// Optional shared detection engine. `None` skips the
    /// detection phase entirely (e.g. for redaction-only or
    /// validation-only pipelines).
    pub detection_engine: Option<Arc<DetectionEngine>>,
    /// Server-wide redaction defaults from `RuntimeConfig.redactor`.
    /// Per-workflow `Redaction` fields fall back to these.
    pub redactor_defaults: Arc<RedactorDefaults>,
    /// Optional limit on how many documents may process concurrently.
    pub concurrency: Option<ConcurrencyPolicy>,
    /// When `true`, skip redaction, validation, and export phases.
    pub dry_run: bool,
}

/// Result of processing a single document through the pipeline.
#[derive(Debug)]
pub(super) struct DocumentResult {
    /// The processed envelope, if the document completed successfully.
    pub envelope: Option<DocumentEnvelope>,
    /// Error message if the document failed, `None` on success.
    pub error: Option<String>,
}

/// Aggregate outcome of executing the full pipeline.
#[derive(Debug)]
pub(super) struct RunOutput {
    /// Results from all processed documents.
    pub results: Vec<DocumentResult>,
}

/// Top-level pipeline orchestrator.
///
/// Imports documents, fans them out to concurrent [`DocumentPipeline`]
/// tasks, exports results, and collects outcomes.
pub(super) struct Orchestrator {
    ctx: Arc<RunContext>,
    semaphore: Option<Arc<Semaphore>>,
}

impl Orchestrator {
    /// Create an orchestrator for the given run.
    pub fn new(ctx: RunContext) -> Self {
        let semaphore = ctx
            .concurrency
            .map(|c| Arc::new(Semaphore::new(c.max_nodes)));
        Self {
            ctx: Arc::new(ctx),
            semaphore,
        }
    }

    /// Execute the compiled plan.
    pub async fn run(&self, plan: &ExecutionPlan) -> Result<RunOutput, Error> {
        let envelopes = self.run_imports(&plan.imports).await?;

        let mut join_set = JoinSet::new();
        for envelope in envelopes {
            let ctx = Arc::clone(&self.ctx);
            let sem = self.semaphore.clone();
            let plan = plan.clone();

            join_set.spawn(async move {
                let _permit = match sem {
                    Some(ref s) => Some(s.acquire().await.expect("semaphore closed")),
                    None => None,
                };

                let pipeline = DocumentPipeline { ctx: ctx.clone() };
                match pipeline.run(envelope, &plan).await {
                    Ok(envelope) => DocumentResult {
                        envelope: Some(envelope),
                        error: None,
                    },
                    Err(e) => DocumentResult {
                        envelope: None,
                        error: Some(e.to_string()),
                    },
                }
            });
        }

        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(doc_result) => results.push(doc_result),
                Err(e) => {
                    let msg = if e.is_panic() {
                        let payload = e.into_panic();
                        let panic_msg = payload
                            .downcast_ref::<&str>()
                            .copied()
                            .or_else(|| payload.downcast_ref::<String>().map(|s| s.as_str()))
                            .unwrap_or("unknown panic");
                        tracing::error!(target: TARGET, panic = panic_msg, "document task panicked");
                        format!("Task panicked: {panic_msg}")
                    } else {
                        tracing::error!(target: TARGET, error = %e, "document task failed");
                        format!("Task failed: {e}")
                    };
                    results.push(DocumentResult {
                        envelope: None,
                        error: Some(msg),
                    });
                }
            }
        }

        Ok(RunOutput { results })
    }

    /// Execute import steps to produce envelopes.
    async fn run_imports(&self, imports: &[ImportStep]) -> Result<Vec<DocumentEnvelope>, Error> {
        let mut envelopes = Vec::new();

        for step in imports {
            let import = ImportFile::new()
                .with_decompression(step.config.decompression)
                .with_decryption(step.config.decryption.clone());

            let shared = &self.ctx.shared;
            for &content_id in &step.config.content_ids {
                tracing::debug!(target: TARGET, %content_id, "importing content");
                let handle = shared
                    .registry
                    .read_content(shared.actor_id, content_id)
                    .await?;
                let content = handle.content().await?;
                let envelope = import.import(content, &self.ctx.shared).await?;
                envelopes.push(envelope);
            }
        }

        tracing::info!(target: TARGET, count = envelopes.len(), "documents imported");
        Ok(envelopes)
    }
}

/// Processes a single document through all plan phases sequentially.
struct DocumentPipeline {
    ctx: Arc<RunContext>,
}

impl DocumentPipeline {
    /// Run all phases for a single envelope.
    async fn run(
        &self,
        mut envelope: DocumentEnvelope,
        plan: &ExecutionPlan,
    ) -> Result<DocumentEnvelope, Error> {
        self.check_cancelled()?;

        // Phase 1: extraction.
        self.run_phase(&plan.extraction_policy, async {
            self.run_extraction(&plan.extraction, &mut envelope).await
        })
        .await?;
        self.check_cancelled()?;

        // Phase 2: detection.
        self.run_phase(&plan.detection_policy, async {
            self.run_detection(&plan.detection, &mut envelope).await
        })
        .await?;
        self.check_cancelled()?;

        // Phase 3: deduplication.
        Deduplication::new(&plan.deduplication)
            .execute(&mut envelope)
            .await?;
        self.check_cancelled()?;

        // Phase 4: redaction + generate context.
        if !self.ctx.dry_run {
            self.run_phase(&plan.redaction_policy, async {
                Redaction::new(&plan.redaction, &self.ctx.redactor_defaults)
                    .execute(&mut envelope)
                    .await
            })
            .await?;
        }
        if plan.generate_context {
            GenerateContext::new(&Default::default())
                .execute(&mut envelope)
                .await?;
        }
        self.check_cancelled()?;

        // Phase 5: validation (skipped in dry-run).
        if !self.ctx.dry_run {
            Validation::new(&plan.validation)
                .execute(&mut envelope)
                .await?;
        }

        // Phase 6: export (skipped in dry-run).
        if !self.ctx.dry_run {
            self.run_exports(&plan.exports, &envelope).await?;
        }

        Ok(envelope)
    }

    /// Wrap a phase future with an optional timeout from the phase policy.
    async fn run_phase<F>(&self, policy: &PhasePolicy, future: F) -> Result<(), Error>
    where
        F: Future<Output = Result<(), Error>> + Send,
    {
        match &policy.timeout {
            Some(tp) => tp.with_timeout(future).await,
            None => future.await,
        }
    }

    /// Run extraction by dispatching the document to the matching
    /// pre-built extractor in [`Extractors`].
    ///
    /// [`Extractors`]: crate::extraction::Extractors
    async fn run_extraction(
        &self,
        cfg: &ExtractionConfig,
        envelope: &mut DocumentEnvelope,
    ) -> Result<(), Error> {
        self.ctx.extractors.run(envelope, cfg).await
    }

    /// Run detection through the run-scoped [`DetectionEngine`].
    ///
    /// The engine is built once per run from `plan.detection` (see
    /// [`Detection::into_engine`]) and stored on [`RunContext`]; it
    /// is `None` when no recognizer is opted in (every per-slot
    /// field on `plan.detection` is `None`), in which case the
    /// detection phase is skipped.
    ///
    /// Per-call hints from the workflow [`DetectionConfig`] —
    /// `cfg.params.entity_kinds` (allowlist) and
    /// `cfg.params.confidence_threshold` — flow into the
    /// `DetectionContext` for each text span. Recognizer-specific
    /// settings are baked into the matching recognizer at engine-
    /// construction time.
    ///
    /// [`Detection::into_engine`]: crate::detection::Detection::into_engine
    async fn run_detection(
        &self,
        cfg: &DetectionConfig,
        envelope: &mut DocumentEnvelope,
    ) -> Result<(), Error> {
        if let Some(ref engine) = self.ctx.detection_engine {
            let op = Detection::new(Arc::clone(engine), cfg.clone());
            op.execute(envelope).await?;
        }
        Ok(())
    }

    /// Export envelopes to the registry.
    async fn run_exports(
        &self,
        exports: &[ExportStep],
        envelope: &DocumentEnvelope,
    ) -> Result<(), Error> {
        for step in exports {
            let export = ExportFile::new()
                .with_encryption(step.config.encryption.clone())
                .with_compression(step.config.compression)
                .with_content_ids(step.config.content_ids.clone());
            export.export(envelope).await?;
        }
        Ok(())
    }

    fn check_cancelled(&self) -> Result<(), Error> {
        if self.ctx.cancel.is_cancelled() {
            return Err(Error::cancellation("run cancelled"));
        }
        Ok(())
    }
}

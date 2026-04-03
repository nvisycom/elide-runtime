//! Pipeline orchestrator: concurrent document processing through a
//! typed execution plan.
//!
//! The [`Orchestrator`] drives the pipeline at the top level: it
//! imports documents, fans them out to concurrent [`DocumentPipeline`]
//! tasks (one per document), and collects the results.
//!
//! [`DocumentPipeline`] processes a single document through all plan
//! phases sequentially: extraction → detection → fusion → redaction →
//! validation. Detection runs NER and Pattern concurrently.

use std::sync::Arc;

use nvisy_core::Error;
use nvisy_ontology::workflow::{ConcurrencyPolicy, Detection, Extraction};
use nvisy_provider::http::HttpClient;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use super::config::RuntimeConfig;
use super::plan::{ExecutionPlan, ExportStep, ImportStep};
use crate::graph::RetryExt;
use crate::operation::envelope::SharedData;
use crate::operation::{
    AudialExtractionOp, DocumentEnvelope, EntityRecognitionOp, ExportFileOp, FusionOp,
    GenerateContextOp, ImportFileOp, Operation, PatternRecognitionOp, RedactionOp,
    SaveContextOp, ValidationOp, VisualExtractionOp,
};

const TARGET: &str = "nvisy_engine::pipeline::orchestrator";

/// Per-run execution context shared across all document tasks.
pub(super) struct RunContext {
    /// Token to signal cancellation to all tasks.
    pub cancel: CancellationToken,
    /// Shared run-wide state: run ID, actor, registry, policies.
    pub shared: Arc<SharedData>,
    /// Effective configuration after merging per-request overrides.
    pub config: Arc<RuntimeConfig>,
    /// Shared HTTP client for downstream API calls.
    pub http_client: HttpClient,
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
    async fn run_imports(
        &self,
        imports: &[ImportStep],
    ) -> Result<Vec<DocumentEnvelope>, Error> {
        let mut envelopes = Vec::new();

        for step in imports {
            let import = ImportFileOp::new()
                .with_decompression(step.config.decompression)
                .with_decryption(step.config.decryption.clone());

            let shared = &self.ctx.shared;
            for &content_id in &step.config.content_ids {
                let do_import = || async {
                    tracing::debug!(target: TARGET, %content_id, "importing content");
                    let handle = shared
                        .registry
                        .read_content(shared.actor_id, content_id)
                        .await?;
                    let content = handle.content().await?;
                    import.import(content, &self.ctx.shared).await
                };

                let envelope = match &step.retry {
                    Some(policy) => policy.with_retry(do_import).await?,
                    None => do_import().await?,
                };
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
        self.run_extraction(&plan.extraction, &mut envelope).await?;
        self.check_cancelled()?;

        // Phase 2: detection (NER + Pattern concurrently).
        self.run_detection(&plan.detection, &mut envelope).await?;
        self.check_cancelled()?;

        // Phase 3: fusion.
        FusionOp::new(&plan.fusion).execute(&mut envelope).await?;
        self.check_cancelled()?;

        // Phase 4: redaction + generate context.
        if !self.ctx.dry_run {
            RedactionOp::new(&plan.redaction)
                .execute(&mut envelope)
                .await?;
        }
        if plan.generate_context {
            GenerateContextOp::new(&Default::default())
                .execute(&mut envelope)
                .await?;
        }
        self.check_cancelled()?;

        // Phase 5: validation (skipped in dry-run).
        if !self.ctx.dry_run {
            ValidationOp::new(&plan.validation)
                .execute(&mut envelope)
                .await?;
        }

        // Phase 6: export (skipped in dry-run).
        if !self.ctx.dry_run {
            self.run_exports(&plan.exports, &envelope).await?;

            // Save contexts.
            for &id in &plan.save_context_ids {
                let cfg = nvisy_ontology::workflow::SaveContext {
                    context_ids: vec![id],
                };
                SaveContextOp::new(&cfg).execute(&mut envelope).await?;
            }
        }

        Ok(envelope)
    }

    /// Run extraction for all applicable modalities.
    async fn run_extraction(
        &self,
        cfg: &Extraction,
        envelope: &mut DocumentEnvelope,
    ) -> Result<(), Error> {
        let visual_cfg = cfg.visual.clone().unwrap_or_default();
        let audial_cfg = cfg.audial.clone().unwrap_or_default();

        // Visual extraction (OCR) — only errors if provider is missing
        // and the document is an image. Silently skip if no provider.
        if let Ok(op) =
            VisualExtractionOp::new(&visual_cfg, &self.ctx.config, &self.ctx.http_client)
        {
            op.execute(envelope).await?;
        }

        // Audial extraction (STT) — same pattern.
        if let Ok(op) =
            AudialExtractionOp::new(&audial_cfg, &self.ctx.config, &self.ctx.http_client)
        {
            op.execute(envelope).await?;
        }

        // Text extraction is a no-op for now (text documents are
        // already text). Future: whitespace normalization, encoding
        // detection, etc.

        Ok(())
    }

    /// Run detection methods sequentially.
    ///
    /// NER and Pattern are logically independent but both mutate the
    /// envelope (appending to `audit.entities`), so they run
    /// sequentially on the same `&mut` reference. NER silently skips
    /// if no LLM provider is configured.
    async fn run_detection(
        &self,
        cfg: &Detection,
        envelope: &mut DocumentEnvelope,
    ) -> Result<(), Error> {
        // NER detection — skip if no LLM provider.
        let ner_cfg = cfg.ner.clone().unwrap_or_default();
        if let Ok(op) =
            EntityRecognitionOp::new(&ner_cfg, &self.ctx.config, &self.ctx.http_client).await
        {
            op.execute(envelope).await?;
        }

        // Pattern detection — always available (no external deps).
        let pat_cfg = cfg.pattern.clone().unwrap_or_default();
        PatternRecognitionOp::new(&pat_cfg)
            .execute(envelope)
            .await?;

        Ok(())
    }

    /// Export envelopes to the registry.
    async fn run_exports(
        &self,
        exports: &[ExportStep],
        envelope: &DocumentEnvelope,
    ) -> Result<(), Error> {
        for step in exports {
            let export = ExportFileOp::new()
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

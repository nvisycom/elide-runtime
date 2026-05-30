//! Pipeline orchestrator: concurrent document processing through a
//! flat fixed-order plan.
//!
//! The [`Orchestrator`] drives the pipeline at the top level: it
//! imports documents, fans them out to concurrent [`DocumentPipeline`]
//! tasks (one per document), and collects the results.
//!
//! [`DocumentPipeline`] processes a single document through all phases
//! sequentially: extraction → detection → deduplication → redaction →
//! validation.

use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::sync::Arc;

use nvisy_core::Error;
use nvisy_ontology::modality::{Audio, Image, Modality, Overlap, Tabular, Text};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use super::default::EngineInput;
use crate::deduplication::{Deduplicator, FilterParams, SpanSize};
use crate::detection::{Detect, DetectionEngine, LiftFromBlock, ProjectIntoBlock};
use crate::envelope::value_at::ValueAt;
use crate::envelope::{AnyEnvelope, DocumentEnvelope, SharedData};
use crate::extraction::{Extract, Extraction, Extractors, WorkflowSlice};
use crate::ingestion::{
    ExportFile as ExportFileConfig, Exporter, ImportFile as ImportFileConfig, Importer,
};
use crate::redaction::{ApplyRedactions, RedactionDefaults, Redactor};
use crate::validation::{CheckLeaks, Validator};

const TARGET: &str = "nvisy_engine::pipeline::orchestrator";

/// Per-run execution context shared across all document tasks.
pub(super) struct RunContext {
    /// Token to signal cancellation to all tasks.
    pub(super) cancel: CancellationToken,
    /// Shared run-wide state: run ID, actor, registry, policies.
    pub(super) shared: Arc<SharedData>,
    /// Pre-built extractor registry from `RuntimeConfig.extraction`.
    /// Shared across every run.
    pub(super) extractors: Arc<Extractors>,
    /// Optional shared detection engine. `None` skips the
    /// detection phase entirely (e.g. for redaction-only or
    /// validation-only pipelines).
    pub(super) detection_engine: Option<Arc<DetectionEngine>>,
    /// Server-wide redaction defaults from `RuntimeConfig.redaction`.
    /// Per-workflow `Redaction` fields fall back to these.
    pub(super) redaction_defaults: Arc<RedactionDefaults>,
    /// Optional limit on how many documents may process concurrently.
    pub(super) concurrency: Option<NonZeroUsize>,
    /// When `true`, skip redaction, validation, and export phases.
    pub(super) dry_run: bool,
}

/// Result of processing a single document through the pipeline.
///
/// The envelope is modality-erased ([`AnyEnvelope`]) so a single
/// `Vec<DocumentResult>` can carry results across every modality the
/// run produced.
#[derive(Debug)]
pub(super) struct DocumentResult {
    /// The processed envelope, if the document completed successfully.
    pub envelope: Option<AnyEnvelope>,
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
        let semaphore = ctx.concurrency.map(|c| Arc::new(Semaphore::new(c.get())));
        Self {
            ctx: Arc::new(ctx),
            semaphore,
        }
    }

    /// Execute the plan.
    pub async fn run(&self, plan: &EngineInput) -> Result<RunOutput, Error> {
        let envelopes = self.run_imports(&plan.imports).await?;

        let mut results: Vec<DocumentResult> = Vec::new();
        let mut join_set: JoinSet<DocumentResult> = JoinSet::new();
        for envelope in envelopes {
            let ctx = Arc::clone(&self.ctx);
            let sem = self.semaphore.clone();
            let plan = plan.clone();

            match envelope {
                AnyEnvelope::Text(env) => {
                    join_set.spawn(run_typed_pipeline(env, ctx, sem, plan, AnyEnvelope::Text));
                }
                AnyEnvelope::Tabular(env) => {
                    join_set.spawn(run_typed_pipeline(
                        env,
                        ctx,
                        sem,
                        plan,
                        AnyEnvelope::Tabular,
                    ));
                }
                AnyEnvelope::Image(env) => {
                    join_set.spawn(run_typed_pipeline(env, ctx, sem, plan, AnyEnvelope::Image));
                }
                AnyEnvelope::Audio(env) => {
                    join_set.spawn(run_typed_pipeline(env, ctx, sem, plan, AnyEnvelope::Audio));
                }
            }
        }

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
    ///
    /// Each imported content can fan out into multiple envelopes
    /// (e.g. rich documents → one text + one image envelope sharing
    /// a single codec handle), so the result is a flat
    /// `Vec<AnyEnvelope>`.
    async fn run_imports(&self, imports: &[ImportFileConfig]) -> Result<Vec<AnyEnvelope>, Error> {
        let mut envelopes = Vec::new();

        for cfg in imports {
            let importer = Importer::new()
                .with_decompression(cfg.decompression)
                .with_decryption(cfg.decryption.clone());

            let shared = &self.ctx.shared;
            for &content_id in &cfg.content_ids {
                tracing::debug!(target: TARGET, %content_id, "importing content");
                let handle = shared
                    .registry
                    .read_content(shared.actor_id, content_id)
                    .await?;
                let content = handle.content().await?;
                envelopes.extend(importer.import(content, &self.ctx.shared).await?);
            }
        }

        tracing::info!(target: TARGET, count = envelopes.len(), "envelopes imported");
        Ok(envelopes)
    }
}

/// Processes a single document through all plan phases sequentially.
///
/// Generic over modality `M`: a single `DocumentPipeline<M>` runs
/// the same stage sequence (extraction → detection → dedup →
/// redaction → validation → export) for any modality. Per-modality
/// behaviour comes from the trait bounds on the `impl` block
/// (`Extract<M>`, `Detect<M>`, `ApplyRedactions`, `CheckLeaks`,
/// `LiftFromBlock`, `ProjectIntoBlock`, `SpanSize`, `Overlap`,
/// `ValueAt<M>`); the stage methods themselves are modality-agnostic.
struct DocumentPipeline<M: Modality> {
    ctx: Arc<RunContext>,
    _marker: PhantomData<fn() -> M>,
}

impl<M: Modality> DocumentPipeline<M> {
    /// Construct a new pipeline for modality `M`.
    fn new(ctx: Arc<RunContext>) -> Self {
        Self {
            ctx,
            _marker: PhantomData,
        }
    }

    /// Cancellation-token guard shared by every stage.
    fn check_cancelled(&self) -> Result<(), Error> {
        if self.ctx.cancel.is_cancelled() {
            return Err(Error::cancellation("run cancelled", "orchestrator"));
        }
        Ok(())
    }
}

/// Per-modality pipeline execution.
///
/// Each modality has its own monomorphized pipeline body — text runs
/// every stage today; image/audio/tabular grow stage bodies in
/// later Scope C steps. The orchestrator's dispatch loop calls into
/// the right specialization through this trait without spelling out
/// every variant by hand.
#[async_trait::async_trait]
trait RunPipeline<M: Modality>: Sized {
    async fn run(
        self,
        envelope: DocumentEnvelope<M>,
        plan: &EngineInput,
    ) -> Result<DocumentEnvelope<M>, Error>;
}

/// Spawn-able task that runs the per-modality pipeline and wraps the
/// result back into [`AnyEnvelope`] for the orchestrator's results
/// vector.
///
/// `wrap` is the corresponding [`AnyEnvelope`] variant constructor
/// (e.g. `AnyEnvelope::Text`); inference picks it up automatically
/// at each call site.
async fn run_typed_pipeline<M, F>(
    envelope: DocumentEnvelope<M>,
    ctx: Arc<RunContext>,
    sem: Option<Arc<Semaphore>>,
    plan: EngineInput,
    wrap: F,
) -> DocumentResult
where
    M: Modality,
    DocumentPipeline<M>: RunPipeline<M>,
    DocumentEnvelope<M>: Send + 'static,
    F: FnOnce(DocumentEnvelope<M>) -> AnyEnvelope + Send + 'static,
{
    let _permit = match sem {
        Some(ref s) => match s.acquire().await {
            Ok(permit) => Some(permit),
            Err(_) => {
                return DocumentResult {
                    envelope: None,
                    error: Some("concurrency semaphore closed".to_owned()),
                };
            }
        },
        None => None,
    };
    let pipeline = DocumentPipeline::<M>::new(ctx);
    match pipeline.run(envelope, &plan).await {
        Ok(env) => DocumentResult {
            envelope: Some(wrap(env)),
            error: None,
        },
        Err(e) => DocumentResult {
            envelope: None,
            error: Some(e.to_string()),
        },
    }
}

impl<M> DocumentPipeline<M>
where
    M: Modality + LiftFromBlock + ProjectIntoBlock + Overlap + SpanSize + CheckLeaks,
    Extractors: Extract<M>,
    Extraction: WorkflowSlice<M>,
    DetectionEngine: Detect<M>,
    DocumentEnvelope<M>: ValueAt<M> + ApplyRedactions + Send,
{
    /// Run extraction → detection → dedup → redaction → validation
    /// → export for one envelope. Generic over modality; per-M
    /// behaviour comes from the trait impls bounded above.
    async fn run_full(
        self,
        mut envelope: DocumentEnvelope<M>,
        plan: &EngineInput,
    ) -> Result<DocumentEnvelope<M>, Error> {
        self.check_cancelled()?;

        // Extraction.
        Extract::<M>::extract(
            self.ctx.extractors.as_ref(),
            &mut envelope,
            plan.extraction.workflow_for::<M>(),
        )
        .await?;
        self.check_cancelled()?;

        // Detection.
        if let Some(ref engine) = self.ctx.detection_engine {
            Detect::<M>::detect(engine.as_ref(), &mut envelope, &plan.detection).await?;
        }
        self.check_cancelled()?;

        // Deduplication.
        let dedup = Deduplicator::new(&plan.deduplication);
        let params = FilterParams {
            allowed_kinds: (!plan.detection.entity_kinds.is_empty())
                .then(|| plan.detection.entity_kinds.clone()),
            confidence_threshold: plan.detection.confidence_threshold,
        };
        dedup.execute(&mut envelope, &params).await?;
        self.check_cancelled()?;

        // Redaction.
        if !self.ctx.dry_run {
            Redactor::new(&plan.redaction, &self.ctx.redaction_defaults)
                .execute(&mut envelope)
                .await?;
        }
        self.check_cancelled()?;

        // Validation (skipped in dry-run).
        if !self.ctx.dry_run {
            Validator::new(&plan.validation)
                .execute(&mut envelope)
                .await?;
        }

        // Export (skipped in dry-run).
        if !self.ctx.dry_run {
            self.run_exports(&plan.exports, &envelope).await?;
        }

        Ok(envelope)
    }

    /// Export the envelope to the registry under every configured
    /// export descriptor.
    async fn run_exports(
        &self,
        exports: &[ExportFileConfig],
        envelope: &DocumentEnvelope<M>,
    ) -> Result<(), Error> {
        for cfg in exports {
            let exporter = Exporter::new()
                .with_encryption(cfg.encryption.clone())
                .with_compression(cfg.compression)
                .with_content_ids(cfg.content_ids.clone());
            exporter.export(envelope).await?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl RunPipeline<Text> for DocumentPipeline<Text> {
    async fn run(
        self,
        envelope: DocumentEnvelope<Text>,
        plan: &EngineInput,
    ) -> Result<DocumentEnvelope<Text>, Error> {
        self.run_full(envelope, plan).await
    }
}

#[async_trait::async_trait]
impl RunPipeline<Tabular> for DocumentPipeline<Tabular> {
    async fn run(
        self,
        envelope: DocumentEnvelope<Tabular>,
        plan: &EngineInput,
    ) -> Result<DocumentEnvelope<Tabular>, Error> {
        self.run_full(envelope, plan).await
    }
}

#[async_trait::async_trait]
impl RunPipeline<Image> for DocumentPipeline<Image> {
    async fn run(
        self,
        envelope: DocumentEnvelope<Image>,
        plan: &EngineInput,
    ) -> Result<DocumentEnvelope<Image>, Error> {
        self.run_full(envelope, plan).await
    }
}

#[async_trait::async_trait]
impl RunPipeline<Audio> for DocumentPipeline<Audio> {
    async fn run(
        self,
        envelope: DocumentEnvelope<Audio>,
        plan: &EngineInput,
    ) -> Result<DocumentEnvelope<Audio>, Error> {
        self.run_full(envelope, plan).await
    }
}

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
use nvisy_ontology::modality::{Modality, Overlap};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use super::default::EngineInput;
use super::phase::{Phase, PhaseContext};
use crate::deduplication::{DeduplicationPhase, SpanSize};
use crate::detection::{
    DetectDispatch, DetectionEngine, DetectionPhase, LiftFromBlock, ProjectIntoBlock,
};
use crate::envelope::value_at::ValueAt;
use crate::envelope::{AnyEnvelope, DocumentEnvelope, SharedData};
use crate::extraction::{
    ExtractDispatch, Extraction, ExtractionEngine, ExtractionPhase, PlanSlice,
};
use crate::ingestion::{
    ExportFile as ExportFileConfig, Exporter, ImportFile as ImportFileConfig, Importer,
};
use crate::redaction::{ApplyRedactions, RedactionConfig, RedactionPhase};
use crate::validation::{CheckLeaks, ValidationPhase};

const TARGET: &str = "nvisy_engine::pipeline::orchestrator";

/// Per-run execution context shared across all document tasks.
pub(crate) struct RunContext {
    /// Token to signal cancellation to all tasks.
    pub(crate) cancel: CancellationToken,
    /// Shared run-wide state: run ID, actor, registry, policies.
    pub(crate) shared: Arc<SharedData>,
    /// Pre-built extractor registry from `RuntimeConfig.extraction`.
    /// Shared across every run.
    pub(crate) extraction_engine: Arc<ExtractionEngine>,
    /// Optional shared detection engine. `None` skips the
    /// detection phase entirely; set when the plan's
    /// `Detection.kinds` is empty (redaction-only flows, no-op
    /// dry runs, test harnesses). See `pipeline/run.rs` —
    /// the engine is built lazily only when at least one
    /// recognizer kind is requested.
    pub(crate) detection_engine: Option<Arc<DetectionEngine>>,
    /// Server-wide redaction defaults from `RuntimeConfig.redaction`.
    /// Per-plan `Redaction` fields fall back to these.
    pub(crate) redaction_config: Arc<RedactionConfig>,
    /// Optional limit on how many documents may process concurrently.
    pub(crate) concurrency: Option<NonZeroUsize>,
    /// When `true`, skip redaction, validation, and export phases.
    pub(crate) dry_run: bool,
}

/// Result of processing a single document through the pipeline.
///
/// The envelope is modality-erased ([`AnyEnvelope`]) so a single
/// `Vec<DocumentResult>` can carry results across every modality the
/// run produced.
#[derive(Debug)]
pub(crate) struct DocumentResult {
    /// The processed envelope, if the document completed successfully.
    pub envelope: Option<AnyEnvelope>,
    /// Error message if the document failed, `None` on success.
    pub error: Option<String>,
}

/// Aggregate outcome of executing the full pipeline.
#[derive(Debug)]
pub(crate) struct RunOutput {
    /// Results from all processed documents.
    pub results: Vec<DocumentResult>,
}

/// Top-level pipeline orchestrator.
///
/// Imports documents, fans them out to concurrent [`DocumentPipeline`]
/// tasks, exports results, and collects outcomes.
pub(crate) struct Orchestrator {
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

    /// Execute the input's plan against every imported document.
    pub async fn run(&self, input: &EngineInput) -> Result<RunOutput, Error> {
        let envelopes = self.run_imports(&input.imports).await?;

        let mut results: Vec<DocumentResult> = Vec::new();
        let mut join_set: JoinSet<DocumentResult> = JoinSet::new();
        for envelope in envelopes {
            let ctx = Arc::clone(&self.ctx);
            let sem = self.semaphore.clone();
            let input = input.clone();

            match envelope {
                AnyEnvelope::Text(env) => {
                    join_set.spawn(run_typed_pipeline(env, ctx, sem, input, AnyEnvelope::Text));
                }
                AnyEnvelope::Tabular(env) => {
                    join_set.spawn(run_typed_pipeline(
                        env,
                        ctx,
                        sem,
                        input,
                        AnyEnvelope::Tabular,
                    ));
                }
                AnyEnvelope::Image(env) => {
                    join_set.spawn(run_typed_pipeline(env, ctx, sem, input, AnyEnvelope::Image));
                }
                AnyEnvelope::Audio(env) => {
                    join_set.spawn(run_typed_pipeline(env, ctx, sem, input, AnyEnvelope::Audio));
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
///
/// The pipeline body lives on a single inherent
/// [`DocumentPipeline::run`] (rather than behind a per-modality
/// dispatch trait): every modality runs the same sequence today
/// and the trait dance bought no per-`M` customization.
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
    input: EngineInput,
    wrap: F,
) -> DocumentResult
where
    M: Modality + LiftFromBlock + ProjectIntoBlock + Overlap + SpanSize + CheckLeaks,
    ExtractionEngine: ExtractDispatch<M>,
    Extraction: PlanSlice<M>,
    DetectionEngine: DetectDispatch<M>,
    DocumentEnvelope<M>: ValueAt<M> + ApplyRedactions + Send + 'static,
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
    match pipeline.run(envelope, &input).await {
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
    ExtractionEngine: ExtractDispatch<M>,
    Extraction: PlanSlice<M>,
    DetectionEngine: DetectDispatch<M>,
    DocumentEnvelope<M>: ValueAt<M> + ApplyRedactions + Send,
{
    /// Walk the per-document phase sequence for one envelope.
    ///
    /// Phase order lives in [`Self::build_phases`]; the loop body
    /// is `cancellation → tracing span → phase.run`. Export and
    /// ingestion stay outside the `Phase<M>` Vec (see
    /// [`crate::pipeline::phase`] module docs).
    async fn run(
        self,
        mut envelope: DocumentEnvelope<M>,
        input: &EngineInput,
    ) -> Result<DocumentEnvelope<M>, Error> {
        let phases = self.build_phases();
        let ctx = PhaseContext::<M>::new(&self.ctx, &input.plan);

        for phase in &phases {
            self.check_cancelled()?;
            let info = phase.inspect();
            let span = tracing::info_span!(
                target: TARGET,
                "phase",
                name = info.name,
                modality = info.modality.as_str(),
                mutating = info.mutating,
            );
            phase.run(&ctx, &mut envelope).instrument(span).await?;
        }
        self.check_cancelled()?;

        // Export stays outside the Vec — it's read-only against the
        // envelope and operates over a list of configs rather than
        // the envelope shape itself.
        if !self.ctx.dry_run {
            self.run_exports(&input.exports, &envelope).await?;
        }

        Ok(envelope)
    }

    /// Assemble the per-envelope phase sequence.
    ///
    /// Order is fixed: extraction → detection? → deduplication →
    /// (redaction → validation)?. Detection is omitted when no
    /// engine is configured; redaction and validation are omitted
    /// on dry-run. Misordering a phase shows up as an edit to this
    /// one function.
    fn build_phases(&self) -> Vec<Box<dyn Phase<M>>> {
        let mut phases: Vec<Box<dyn Phase<M>>> = Vec::with_capacity(5);

        phases.push(Box::new(ExtractionPhase::<M>::new()));

        if self.ctx.detection_engine.is_some() {
            phases.push(Box::new(DetectionPhase::<M>::new()));
        }

        phases.push(Box::new(DeduplicationPhase::<M>::new()));

        if !self.ctx.dry_run {
            phases.push(Box::new(RedactionPhase::<M>::new()));
            phases.push(Box::new(ValidationPhase::<M>::new()));
        }

        phases
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

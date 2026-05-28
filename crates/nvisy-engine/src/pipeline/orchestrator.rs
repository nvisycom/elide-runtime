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
use nvisy_ontology::modality::{Modality, Text};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use super::default::EngineInput;
use crate::deduplication::{Deduplicator, FilterParams};
use crate::detection::{Detection as DetectionConfig, DetectionEngine};
use crate::envelope::{AnyEnvelope, DocumentEnvelope, SharedData};
use crate::extraction::{Extraction as ExtractionConfig, Extractors};
use crate::ingestion::{
    ExportFile as ExportFileConfig, Exporter, ImportFile as ImportFileConfig, Importer,
};
use crate::redaction::{RedactionDefaults, Redactor};
use crate::validation::Validator;

const TARGET: &str = "nvisy_engine::pipeline::orchestrator";

/// Per-run execution context shared across all document tasks.
pub(super) struct RunContext {
    /// Token to signal cancellation to all tasks.
    pub cancel: CancellationToken,
    /// Shared run-wide state: run ID, actor, registry, policies.
    pub shared: Arc<SharedData>,
    /// Pre-built extractor registry from `RuntimeConfig.extraction`.
    /// Shared across every run.
    pub extractors: Arc<Extractors>,
    /// Optional shared detection engine. `None` skips the
    /// detection phase entirely (e.g. for redaction-only or
    /// validation-only pipelines).
    pub detection_engine: Option<Arc<DetectionEngine>>,
    /// Server-wide redaction defaults from `RuntimeConfig.redaction`.
    /// Per-workflow `Redaction` fields fall back to these.
    pub redaction_defaults: Arc<RedactionDefaults>,
    /// Optional limit on how many documents may process concurrently.
    pub concurrency: Option<NonZeroUsize>,
    /// When `true`, skip redaction, validation, and export phases.
    pub dry_run: bool,
}

/// Result of processing a single document through the pipeline.
#[derive(Debug)]
pub(super) struct DocumentResult {
    /// The processed envelope, if the document completed successfully.
    pub envelope: Option<DocumentEnvelope<Text>>,
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

        let mut results = Vec::new();
        let mut join_set = JoinSet::new();
        for envelope in envelopes {
            let text_envelope = match envelope {
                AnyEnvelope::Text(env) => env,
                other => {
                    let modality = other.modality_name();
                    tracing::warn!(
                        target: TARGET,
                        modality,
                        "non-text envelope skipped; downstream pipeline is text-only \
                         (extraction fan-out for image/audio/tabular is pending)",
                    );
                    results.push(DocumentResult {
                        envelope: None,
                        error: Some(format!(
                            "{modality} modality not yet supported by the pipeline; \
                             only text envelopes are processed"
                        )),
                    });
                    continue;
                }
            };

            let ctx = Arc::clone(&self.ctx);
            let sem = self.semaphore.clone();
            let plan = plan.clone();

            join_set.spawn(async move {
                let _permit = match sem {
                    Some(ref s) => Some(s.acquire().await.expect("semaphore closed")),
                    None => None,
                };

                let pipeline = DocumentPipeline::<Text> {
                    ctx: ctx.clone(),
                    _marker: PhantomData,
                };
                match pipeline.run(text_envelope, &plan).await {
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
/// Generic over modality `M`: a single `DocumentPipeline<M>` runs the
/// same stage sequence (extraction → detection → dedup → redaction →
/// validation → export) for any modality. Today only the `<Text>`
/// specialization carries stage bodies; per-modality stage methods
/// are added by Scope C in later steps (§4.3 onward).
struct DocumentPipeline<M: Modality> {
    ctx: Arc<RunContext>,
    _marker: PhantomData<fn() -> M>,
}

impl<M: Modality> DocumentPipeline<M> {
    /// Cancellation-token guard shared by every stage.
    fn check_cancelled(&self) -> Result<(), Error> {
        if self.ctx.cancel.is_cancelled() {
            return Err(Error::cancellation("run cancelled"));
        }
        Ok(())
    }
}

impl DocumentPipeline<Text> {
    /// Run all phases for a single text envelope.
    async fn run(
        &self,
        mut envelope: DocumentEnvelope<Text>,
        plan: &EngineInput,
    ) -> Result<DocumentEnvelope<Text>, Error> {
        self.check_cancelled()?;

        // Extraction.
        self.run_extraction(&plan.extraction, &mut envelope).await?;
        self.check_cancelled()?;

        // Detection.
        self.run_detection(&plan.detection, &mut envelope).await?;
        self.check_cancelled()?;

        // DeduplicationParams.
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

    /// Run extraction by dispatching the document to the matching
    /// pre-built extractor in [`Extractors`].
    ///
    /// [`Extractors`]: crate::extraction::Extractors
    async fn run_extraction(
        &self,
        cfg: &ExtractionConfig,
        envelope: &mut DocumentEnvelope<Text>,
    ) -> Result<(), Error> {
        self.ctx.extractors.run(envelope, cfg).await
    }

    /// Run detection through the shared [`DetectionEngine`].
    ///
    /// The engine is built once per run by [`Pipeline`]
    /// from `input.detection` and stored on [`RunContext`]; it is
    /// `None` when no recognizer is opted in (`detection.kinds` is
    /// empty), in which case detection is skipped.
    ///
    /// [`Pipeline`]: super::run::Pipeline
    async fn run_detection(
        &self,
        cfg: &DetectionConfig,
        envelope: &mut DocumentEnvelope<Text>,
    ) -> Result<(), Error> {
        if let Some(ref engine) = self.ctx.detection_engine {
            engine.detect_in(envelope, cfg).await?;
        }
        Ok(())
    }

    /// Export envelopes to the registry.
    async fn run_exports(
        &self,
        exports: &[ExportFileConfig],
        envelope: &DocumentEnvelope<Text>,
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

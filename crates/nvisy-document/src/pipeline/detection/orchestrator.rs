//! Detection orchestrator: imports content, fans out per-document
//! detection tasks, collects audits.
//!
//! Mirrors the layout of the legacy unified orchestrator
//! (`pipeline/orchestrator.rs`) but runs only the
//! detection-suffix phases. The legacy orchestrator stays in
//! place until `Engine::run` is deleted (task #462).

use std::sync::Arc;

use nvisy_core::Error;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use super::document::DetectionDocumentPipeline;
use crate::core::{AnyTree, RunContext};
use crate::phases::ingestion::{ImportFile, Importer};
use crate::pipeline::engine::EngineInput;
use crate::provenance::AnyAudit;

const TARGET: &str = "nvisy_document::pipeline::detection::orchestrator";

/// Per-document outcome of one detection task.
pub(super) struct DetectionDocumentResult {
    /// The processed tree, when the document completed
    /// successfully.
    pub tree: Option<AnyTree>,
    /// Error message when the document failed, `None` on success.
    pub error: Option<String>,
}

/// Aggregate outcome of a detection orchestrator run.
pub(super) struct DetectionOutput {
    pub results: Vec<DetectionDocumentResult>,
}

impl DetectionOutput {
    /// Project the per-document results into the audit shape the
    /// `DetectionState` wants. Documents without a tree (failed)
    /// are excluded from the audit; their error message stays on
    /// `DetectionDocumentResult` for the caller to record.
    pub(super) fn into_audits(self) -> (Vec<AnyAudit>, u64, bool, bool) {
        let mut audits = Vec::new();
        let mut entities = 0u64;
        let mut any_ok = false;
        let mut any_failed = false;
        for result in self.results {
            if result.error.is_some() {
                any_failed = true;
                continue;
            }
            any_ok = true;
            if let Some(tree) = result.tree {
                let audit = tree.audit_cloned();
                entities += audit.entities_count() as u64;
                audits.push(audit);
            }
        }
        (audits, entities, any_ok, any_failed)
    }
}

/// Top-level orchestrator for one detection pass.
pub(super) struct DetectionOrchestrator {
    ctx: Arc<RunContext>,
    semaphore: Option<Arc<Semaphore>>,
}

impl DetectionOrchestrator {
    pub(super) fn new(ctx: RunContext) -> Self {
        let semaphore = ctx.concurrency().map(|c| Arc::new(Semaphore::new(c.get())));
        Self {
            ctx: Arc::new(ctx),
            semaphore,
        }
    }

    /// Run detection against every imported document.
    ///
    /// `input` is the legacy `EngineInput` shape — see the
    /// architecture doc for why; the orchestrator only reads
    /// `imports` and `plan` from it. Exports / dry_run flags are
    /// ignored.
    pub(super) async fn run(
        &self,
        input: &EngineInput,
    ) -> Result<DetectionOutput, Error> {
        let trees = self.run_imports(&input.imports).await?;

        let pipeline = Arc::new(DetectionDocumentPipeline::from_context(&self.ctx));
        let input = Arc::new(input.clone());
        let mut join_set: JoinSet<DetectionDocumentResult> = JoinSet::new();
        for tree in trees {
            let ctx = Arc::clone(&self.ctx);
            let sem = self.semaphore.clone();
            let input = Arc::clone(&input);
            let pipeline = Arc::clone(&pipeline);
            join_set.spawn(run_one(pipeline, tree, ctx, sem, input));
        }

        let mut results: Vec<DetectionDocumentResult> = Vec::new();
        while let Some(joined) = join_set.join_next().await {
            match joined {
                Ok(doc) => results.push(doc),
                Err(e) => {
                    let msg = if e.is_panic() {
                        let payload = e.into_panic();
                        let panic_msg = payload
                            .downcast_ref::<&str>()
                            .copied()
                            .or_else(|| payload.downcast_ref::<String>().map(|s| s.as_str()))
                            .unwrap_or("unknown panic");
                        tracing::error!(
                            target: TARGET,
                            panic = panic_msg,
                            "detection document task panicked"
                        );
                        format!("Detection task panicked: {panic_msg}")
                    } else {
                        tracing::error!(
                            target: TARGET,
                            error = %e,
                            "detection document task failed"
                        );
                        format!("Detection task failed: {e}")
                    };
                    results.push(DetectionDocumentResult {
                        tree: None,
                        error: Some(msg),
                    });
                }
            }
        }
        Ok(DetectionOutput { results })
    }

    /// Import content into typed `AnyTree`s. Cancellation aborts
    /// before per-document tasks fan out.
    async fn run_imports(&self, imports: &[ImportFile]) -> Result<Vec<AnyTree>, Error> {
        let mut trees = Vec::new();
        for cfg in imports {
            let importer = Importer::new()
                .with_decompression(cfg.decompression)
                .with_decryption(cfg.decryption.clone());
            let shared = self.ctx.shared();
            for &content_id in &cfg.content_ids {
                tracing::debug!(target: TARGET, %content_id, "importing content");
                if self.ctx.is_cancelled() {
                    return Err(Error::cancellation("detection cancelled", TARGET));
                }
                let handle = shared
                    .registry
                    .read_content(shared.actor_id, content_id)
                    .await?;
                let content = handle.content().await?;
                trees.extend(importer.import(content, shared).await?);
            }
        }
        tracing::info!(target: TARGET, count = trees.len(), "trees imported");
        Ok(trees)
    }
}

async fn run_one(
    pipeline: Arc<DetectionDocumentPipeline>,
    tree: AnyTree,
    ctx: Arc<RunContext>,
    sem: Option<Arc<Semaphore>>,
    input: Arc<EngineInput>,
) -> DetectionDocumentResult {
    let _permit = match sem {
        Some(s) => match Arc::clone(&s).acquire_owned().await {
            Ok(permit) => Some(permit),
            Err(_) => {
                return DetectionDocumentResult {
                    tree: None,
                    error: Some("concurrency semaphore closed".to_owned()),
                };
            }
        },
        None => None,
    };

    let mut tree = tree;
    let result = match &mut tree {
        AnyTree::Text(t) => pipeline.run_text(&ctx, &input, t).await,
        AnyTree::Tabular(t) => pipeline.run_tabular(&ctx, &input, t).await,
        AnyTree::Image(t) => pipeline.run_image(&ctx, &input, t).await,
        AnyTree::Audio(t) => pipeline.run_audio(&ctx, &input, t).await,
    };
    if let Err(e) = result {
        return DetectionDocumentResult {
            tree: None,
            error: Some(e.to_string()),
        };
    }
    DetectionDocumentResult {
        tree: Some(tree),
        error: None,
    }
}

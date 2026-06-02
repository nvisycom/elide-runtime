//! Pipeline orchestrator: concurrent document processing through a
//! flat fixed-order plan.
//!
//! The [`Orchestrator`] drives the pipeline at the top level: it
//! imports documents into [`DocumentTree`]s, fans them out to
//! concurrent per-document tasks, and collects the results. Each
//! per-document task runs a single shared [`DocumentPipeline`]
//! against the tree.
//!
//! [`DocumentPipeline`] walks every tree node (root + nested
//! embeds) and dispatches the right per-modality body for each
//! phase.

use std::sync::Arc;

use nvisy_core::Error;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use super::document_pipeline::DocumentPipeline;
use super::engine::EngineInput;
use crate::core::{DocumentTree, RunContext};
use crate::phases::ingestion::{Exporter, ImportFile as ImportFileConfig, Importer};

const TARGET: &str = "nvisy_engine::pipeline::orchestrator";

/// Result of processing a single document through the pipeline.
pub(crate) struct DocumentResult {
    /// The processed tree, if the document completed successfully.
    pub tree: Option<DocumentTree>,
    /// Error message if the document failed, `None` on success.
    pub error: Option<String>,
}

/// Aggregate outcome of executing the full pipeline.
pub(crate) struct RunOutput {
    /// Results from all processed documents.
    pub results: Vec<DocumentResult>,
}

/// Top-level pipeline orchestrator.
pub(crate) struct Orchestrator {
    ctx: Arc<RunContext>,
    semaphore: Option<Arc<Semaphore>>,
}

impl Orchestrator {
    /// Create an orchestrator for the given run.
    pub fn new(ctx: RunContext) -> Self {
        let semaphore = ctx.concurrency().map(|c| Arc::new(Semaphore::new(c.get())));
        Self {
            ctx: Arc::new(ctx),
            semaphore,
        }
    }

    /// Execute the input's plan against every imported document.
    pub async fn run(&self, input: &EngineInput) -> Result<RunOutput, Error> {
        let trees = self.run_imports(&input.imports).await?;

        // Build the per-document pipeline once and share it across
        // every spawned task (phases are cheap to clone — they hold
        // `Arc`s to the long-lived engines).
        let pipeline = Arc::new(DocumentPipeline::from_context(&self.ctx));

        let mut results: Vec<DocumentResult> = Vec::new();
        let mut join_set: JoinSet<DocumentResult> = JoinSet::new();
        let input = Arc::new(input.clone());
        for tree in trees {
            let ctx = Arc::clone(&self.ctx);
            let sem = self.semaphore.clone();
            let input = Arc::clone(&input);
            let pipeline = Arc::clone(&pipeline);
            join_set.spawn(run_one(pipeline, tree, ctx, sem, input));
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
                        tree: None,
                        error: Some(msg),
                    });
                }
            }
        }

        Ok(RunOutput { results })
    }

    /// Execute import steps to produce trees.
    ///
    /// Each imported content yields exactly one [`DocumentTree`];
    /// rich documents land as `AnyDocument::Text` with their image
    /// content as nested embeds populated later by extraction.
    async fn run_imports(&self, imports: &[ImportFileConfig]) -> Result<Vec<DocumentTree>, Error> {
        let mut trees = Vec::new();

        for cfg in imports {
            let importer = Importer::new()
                .with_decompression(cfg.decompression)
                .with_decryption(cfg.decryption.clone());

            let shared = self.ctx.shared();
            for &content_id in &cfg.content_ids {
                tracing::debug!(target: TARGET, %content_id, "importing content");
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

/// Spawn-able task that runs the document pipeline against one tree.
async fn run_one(
    pipeline: Arc<DocumentPipeline>,
    mut tree: DocumentTree,
    ctx: Arc<RunContext>,
    sem: Option<Arc<Semaphore>>,
    input: Arc<EngineInput>,
) -> DocumentResult {
    let _permit = match sem {
        Some(s) => match Arc::clone(&s).acquire_owned().await {
            Ok(permit) => Some(permit),
            Err(_) => {
                return DocumentResult {
                    tree: None,
                    error: Some("concurrency semaphore closed".to_owned()),
                };
            }
        },
        None => None,
    };
    if let Err(e) = pipeline.run(&ctx, &input, &mut tree).await {
        return DocumentResult {
            tree: None,
            error: Some(e.to_string()),
        };
    }

    if !ctx.dry_run() {
        for cfg in &input.exports {
            let exporter = Exporter::new()
                .with_encryption(cfg.encryption.clone())
                .with_compression(cfg.compression)
                .with_content_ids(cfg.content_ids.clone());
            if let Err(e) = exporter.export(&tree, ctx.shared()).await {
                return DocumentResult {
                    tree: None,
                    error: Some(e.to_string()),
                };
            }
        }
    }

    DocumentResult {
        tree: Some(tree),
        error: None,
    }
}

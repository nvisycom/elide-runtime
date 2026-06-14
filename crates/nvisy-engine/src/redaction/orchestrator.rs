//! Redaction orchestrator: re-imports content from the
//! detection's import refs, replays audits onto the freshly-built
//! trees, fans out per-document redaction tasks, runs export.
//!
//! The orchestrator deliberately does no recognition / detection /
//! deduplication — the audit from the detection IS the source of
//! truth for what entities to redact. Overrides have already been
//! applied to that audit by [`apply_overrides`] before this
//! orchestrator sees it.
//!
//! [`apply_overrides`]: super::applicator::apply_overrides

use std::collections::HashMap;
use std::sync::Arc;

use nvisy_core::Error;
use nvisy_core::entity::ContentSource;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use super::document::RedactionDocumentPipeline;
use crate::core::ingestion::{ExportFile, Exporter, ImportFile, Importer};
use crate::core::{AnyTree, PhaseContext as _, RedactionContext};
use crate::document::provenance::AnyAudit;
use crate::redaction::RedactionPlan;

const TARGET: &str = "nvisy_engine::pipeline::redaction::orchestrator";

pub(super) struct RedactionDocumentResult {
    pub tree: Option<AnyTree>,
    pub error: Option<String>,
}

pub(super) struct RedactionOutput {
    pub results: Vec<RedactionDocumentResult>,
}

impl RedactionOutput {
    pub(super) fn into_audits(self) -> (Vec<AnyAudit>, u64, bool, bool) {
        let mut audits = Vec::new();
        let mut applied = 0u64;
        let mut any_ok = false;
        let mut any_failed = false;
        for r in self.results {
            if r.error.is_some() {
                any_failed = true;
                continue;
            }
            any_ok = true;
            if let Some(tree) = r.tree {
                let audit = tree.audit_cloned();
                applied += audit.applied_redactions_count() as u64;
                audits.push(audit);
            }
        }
        (audits, applied, any_ok, any_failed)
    }
}

pub(super) struct RedactionOrchestrator {
    ctx: Arc<RedactionContext>,
    semaphore: Option<Arc<Semaphore>>,
}

impl RedactionOrchestrator {
    pub(super) fn new(ctx: RedactionContext) -> Self {
        let semaphore = ctx.concurrency().map(|c| Arc::new(Semaphore::new(c.get())));
        Self {
            ctx: Arc::new(ctx),
            semaphore,
        }
    }

    /// Run the redaction pass.
    ///
    /// `imports` are re-opened from the registry; `exports` are
    /// the sinks the caller wants written; `plan` carries the
    /// per-phase knobs; `prepared_audits` is the detection's
    /// audit with overrides already applied.
    pub(super) async fn run(
        &self,
        imports: &[ImportFile],
        exports: &[ExportFile],
        plan: &RedactionPlan,
        prepared_audits: Vec<AnyAudit>,
    ) -> Result<RedactionOutput, Error> {
        let trees = self.run_imports(imports).await?;
        let trees = replay_audits_into_trees(trees, prepared_audits)?;

        let pipeline = Arc::new(RedactionDocumentPipeline::from_context(&self.ctx));
        let plan = Arc::new(plan.clone());
        let exports = Arc::new(exports.to_vec());
        let mut join_set: JoinSet<RedactionDocumentResult> = JoinSet::new();
        for tree in trees {
            let ctx = Arc::clone(&self.ctx);
            let sem = self.semaphore.clone();
            let plan = Arc::clone(&plan);
            let exports = Arc::clone(&exports);
            let pipeline = Arc::clone(&pipeline);
            join_set.spawn(run_one(pipeline, tree, ctx, sem, plan, exports));
        }

        let mut results = Vec::new();
        while let Some(joined) = join_set.join_next().await {
            match joined {
                Ok(r) => results.push(r),
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
                            "redaction document task panicked"
                        );
                        format!("Redaction task panicked: {panic_msg}")
                    } else {
                        tracing::error!(target: TARGET, error = %e, "redaction document task failed");
                        format!("Redaction task failed: {e}")
                    };
                    results.push(RedactionDocumentResult {
                        tree: None,
                        error: Some(msg),
                    });
                }
            }
        }
        Ok(RedactionOutput { results })
    }

    async fn run_imports(&self, imports: &[ImportFile]) -> Result<Vec<AnyTree>, Error> {
        let mut trees = Vec::new();
        for cfg in imports {
            let importer = Importer::new()
                .with_decompression(cfg.decompression)
                .with_decryption(cfg.decryption.clone());
            let shared = self.ctx.shared();
            for &content_id in &cfg.content_ids {
                if self.ctx.is_cancelled() {
                    return Err(Error::cancellation("redaction cancelled", TARGET));
                }
                tracing::debug!(target: TARGET, %content_id, "re-importing content for redaction");
                let handle = shared
                    .registry
                    .read_content(shared.actor_id, content_id)
                    .await?;
                let content = handle.content().await?;
                let annotations = shared
                    .registry
                    .load_annotations(shared.actor_id, content_id)
                    .await?;
                trees.extend(importer.import(content, annotations, shared).await?);
            }
        }
        Ok(trees)
    }
}

/// Replay a list of `AnyAudit` onto a list of `AnyTree`, matching
/// each audit to a tree of matching modality and source.
///
/// # Errors
///
/// [`ErrorKind::Validation`] when an audit's source has no
/// matching tree (the detection saw an envelope the redaction
/// re-import didn't produce — content drifted or import
/// semantics changed).
///
/// [`ErrorKind::Validation`]: nvisy_core::ErrorKind::Validation
fn replay_audits_into_trees(
    trees: Vec<AnyTree>,
    audits: Vec<AnyAudit>,
) -> Result<Vec<AnyTree>, Error> {
    let mut by_key: HashMap<(ContentSource, &'static str), usize> = HashMap::new();
    for (idx, tree) in trees.iter().enumerate() {
        let source = match tree {
            AnyTree::Text(t) => t.root.audit.source,
            AnyTree::Tabular(t) => t.root.audit.source,
            AnyTree::Image(t) => t.root.audit.source,
            AnyTree::Audio(t) => t.root.audit.source,
        };
        by_key.insert((source, tree.modality_name()), idx);
    }

    let mut trees = trees;
    for audit in audits {
        let (source, modality_name) = match &audit {
            AnyAudit::Text(a) => (a.source, "text"),
            AnyAudit::Tabular(a) => (a.source, "tabular"),
            AnyAudit::Image(a) => (a.source, "image"),
            AnyAudit::Audio(a) => (a.source, "audio"),
        };
        let Some(&idx) = by_key.get(&(source, modality_name)) else {
            return Err(Error::validation(
                format!(
                    "audit for source {source:?} ({modality_name}) has no matching tree after re-import",
                ),
                TARGET,
            ));
        };
        match (&mut trees[idx], audit) {
            (AnyTree::Text(t), AnyAudit::Text(a)) => t.root.audit = a,
            (AnyTree::Tabular(t), AnyAudit::Tabular(a)) => t.root.audit = a,
            (AnyTree::Image(t), AnyAudit::Image(a)) => t.root.audit = a,
            (AnyTree::Audio(t), AnyAudit::Audio(a)) => t.root.audit = a,
            _ => unreachable!("modality matched via key lookup"),
        }
    }
    Ok(trees)
}

async fn run_one(
    pipeline: Arc<RedactionDocumentPipeline>,
    tree: AnyTree,
    ctx: Arc<RedactionContext>,
    sem: Option<Arc<Semaphore>>,
    plan: Arc<RedactionPlan>,
    exports: Arc<Vec<ExportFile>>,
) -> RedactionDocumentResult {
    let _permit = match sem {
        Some(s) => match Arc::clone(&s).acquire_owned().await {
            Ok(permit) => Some(permit),
            Err(_) => {
                return RedactionDocumentResult {
                    tree: None,
                    error: Some("concurrency semaphore closed".to_owned()),
                };
            }
        },
        None => None,
    };

    let mut tree = tree;
    let phase_result = match &mut tree {
        AnyTree::Text(t) => pipeline.run_text(&ctx, &plan, t).await,
        AnyTree::Tabular(t) => pipeline.run_tabular(&ctx, &plan, t).await,
        AnyTree::Image(t) => pipeline.run_image(&ctx, &plan, t).await,
        AnyTree::Audio(t) => pipeline.run_audio(&ctx, &plan, t).await,
    };
    if let Err(e) = phase_result {
        return RedactionDocumentResult {
            tree: None,
            error: Some(e.to_string()),
        };
    }

    for cfg in exports.iter() {
        if ctx.is_cancelled() {
            return RedactionDocumentResult {
                tree: None,
                error: Some("redaction cancelled before export".to_owned()),
            };
        }
        let exporter = Exporter::new()
            .with_encryption(cfg.encryption.clone())
            .with_compression(cfg.compression)
            .with_content_ids(cfg.content_ids.clone());
        if let Err(e) = exporter.export(&tree, ctx.shared()).await {
            return RedactionDocumentResult {
                tree: None,
                error: Some(e.to_string()),
            };
        }
    }
    RedactionDocumentResult {
        tree: Some(tree),
        error: None,
    }
}

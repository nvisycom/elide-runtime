//! Node-level execution: operation dispatch and envelope flow.
//!
//! [`NodeExecutor`] runs a single [`ResolvedNode`] within a pipeline.
//! It matches on [`GraphNodeKind`], constructs the appropriate
//! [`Operation`], and drives the envelope processing loop:
//!
//! 1. **Receive**: pull [`DocumentEnvelope`] from upstream MPSC channels.
//! 2. **Call**: invoke `operation.execute(&mut envelope)`.
//! 3. **Send**: forward the envelope to all downstream channels.
//!
//! Envelope transport (fan-in, fan-out, cloning) lives in the
//! [`transport`] module.
//!
//! [`NodeOutput`] and [`RunOutput`] carry per-node and per-run results
//! back to the [orchestrator] and [`Engine`] for finalization.
//!
//! [`transport`]: super::transport
//! [`DocumentEnvelope`]: crate::operation::DocumentEnvelope
//! [orchestrator]: super::orchestrator
//! [`Engine`]: super::Engine

use std::sync::Arc;

use nvisy_core::{Error, ErrorKind};
use nvisy_ontology::workflow::{
    ExportFile, GraphNodeKind, ImportFile, RetryPolicy, TimeoutBehavior, TimeoutPolicy,
};
use tokio::sync::mpsc;

use super::orchestrator::RunContext;
use super::plan::ResolvedNode;
use super::runs::RunStatus;
use super::transport::{fan_out, forward_envelopes, process_envelopes};
use crate::graph::{RetryExt, TimeoutExt};
use crate::operation::{
    AudialExtractionOp, DocumentEnvelope, EntityRecognitionOp, ExportFileOp, FusionOp,
    GenerateContextOp, ImportFileOp, LoadContextOp, Operation, PatternRecognitionOp, RedactionOp,
    SaveContextOp, ValidationOp, VisualExtractionOp,
};

const TARGET: &str = "nvisy_engine::pipeline::executor";

/// Outcome of executing a single node.
#[derive(Debug)]
pub(super) struct NodeOutput {
    /// Number of envelopes processed by this node.
    pub items_processed: u64,
    /// Error message if the node failed, `None` on success.
    pub error: Option<String>,
    /// Completed envelopes (populated only by export nodes for output collection).
    pub envelopes: Vec<DocumentEnvelope>,
}

/// Aggregate outcome of executing the full DAG.
#[derive(Debug)]
pub(super) struct RunOutput {
    /// Results from all executed nodes (order is non-deterministic).
    pub node_results: Vec<NodeOutput>,
}

impl RunOutput {
    /// Determine overall run status from node results.
    ///
    /// If all errors are cancellation errors, returns [`Cancelled`].
    /// Otherwise uses the standard ok/err breakdown.
    ///
    /// [`Cancelled`]: RunStatus::Cancelled
    pub fn run_status(&self) -> RunStatus {
        let any_ok = self.node_results.iter().any(|r| r.error.is_none());
        let errors: Vec<_> = self
            .node_results
            .iter()
            .filter_map(|r| r.error.as_deref())
            .collect();

        if errors.is_empty() {
            return RunStatus::Succeeded;
        }

        let all_cancelled = errors.iter().all(|e| e.contains("cancelled"));
        if all_cancelled {
            return RunStatus::Cancelled;
        }

        if any_ok {
            RunStatus::PartialFailure
        } else {
            RunStatus::Failed
        }
    }
}

/// Executes a single [`ResolvedNode`] within a pipeline run.
///
/// Holds a shared reference to the [`RunContext`]. The orchestrator
/// creates one executor per node task.
pub(super) struct NodeExecutor {
    ctx: Arc<RunContext>,
}

impl NodeExecutor {
    pub fn new(ctx: Arc<RunContext>) -> Self {
        Self { ctx }
    }

    /// Resolve the effective retry policy: node-level wins, then engine default.
    fn effective_retry(
        &self,
        node: &nvisy_ontology::workflow::GraphNode,
    ) -> Option<RetryPolicy> {
        node.retry().or(self.ctx.config.default_retry()).cloned()
    }

    /// Resolve the effective timeout policy: node-level wins, then engine default.
    fn effective_timeout(
        &self,
        node: &nvisy_ontology::workflow::GraphNode,
    ) -> Option<TimeoutPolicy> {
        node.timeout()
            .or(self.ctx.config.default_timeout())
            .cloned()
    }

    /// Execute a resolved node, applying timeout and cancellation policies.
    pub async fn execute(
        &self,
        resolved: &ResolvedNode,
        senders: Vec<mpsc::Sender<DocumentEnvelope>>,
        mut receivers: Vec<mpsc::Receiver<DocumentEnvelope>>,
    ) -> Result<NodeOutput, Error> {
        if self.ctx.cancel.is_cancelled() {
            return Err(Error::cancellation("run cancelled"));
        }

        // In dry-run mode, skip validation and export phases.
        // Forward envelopes so downstream nodes (if any) still unblock.
        if self.ctx.dry_run && resolved.node.kind.is_post_redaction() {
            tracing::debug!(
                target: TARGET,
                node = %resolved.node.kind,
                "skipping node (dry-run mode)",
            );
            let count = forward_envelopes(&senders, &mut receivers).await?;
            return Ok(NodeOutput {
                items_processed: count,
                error: None,
                envelopes: Vec::new(),
            });
        }

        let retry = self.effective_retry(&resolved.node);
        let timeout = self.effective_timeout(&resolved.node);
        let cancel = self.ctx.cancel.clone();

        let run = async {
            tokio::select! {
                _ = cancel.cancelled() => Err(Error::cancellation("run cancelled")),
                result = self.dispatch(
                    &resolved.node.kind,
                    retry,
                    &senders,
                    &mut receivers,
                ) => result,
            }
        };

        match &timeout {
            Some(tp) => {
                let result: Result<NodeOutput, Error> = tp.with_timeout(run).await;
                match (&result, &tp.on_timeout) {
                    (Err(e), TimeoutBehavior::Skip) if e.kind == ErrorKind::Timeout => {
                        Ok(NodeOutput {
                            items_processed: 0,
                            error: None,
                            envelopes: Vec::new(),
                        })
                    }
                    _ => result,
                }
            }
            None => run.await,
        }
    }

    /// Route a node to its operation-specific handler based on [`GraphNodeKind`].
    async fn dispatch(
        &self,
        kind: &GraphNodeKind,
        retry: Option<RetryPolicy>,
        senders: &[mpsc::Sender<DocumentEnvelope>],
        receivers: &mut [mpsc::Receiver<DocumentEnvelope>],
    ) -> Result<NodeOutput, Error> {
        match kind {
            GraphNodeKind::ImportFile(cfg) => {
                self.execute_import(cfg, retry.as_ref(), senders).await
            }
            GraphNodeKind::ExportFile(cfg) => self.execute_export(cfg, receivers).await,
            GraphNodeKind::Extraction(cfg) => {
                // Run applicable modalities sequentially.
                let visual_cfg = cfg.visual.clone().unwrap_or_default();
                let audial_cfg = cfg.audial.clone().unwrap_or_default();
                // Visual
                if let Ok(op) =
                    VisualExtractionOp::new(&visual_cfg, &self.ctx.config, &self.ctx.http_client)
                {
                    self.execute_op(op, senders, receivers).await?;
                }
                // Audial
                if let Ok(op) =
                    AudialExtractionOp::new(&audial_cfg, &self.ctx.config, &self.ctx.http_client)
                {
                    self.execute_op(op, senders, receivers).await?;
                }
                Ok(node_output(0))
            }
            GraphNodeKind::Detection(cfg) => {
                // Run NER and Pattern sequentially (concurrent version comes in Part 4).
                let ner_cfg = cfg.ner.clone().unwrap_or_default();
                let pat_cfg = cfg.pattern.clone().unwrap_or_default();
                if let Ok(op) =
                    EntityRecognitionOp::new(&ner_cfg, &self.ctx.config, &self.ctx.http_client)
                        .await
                {
                    self.execute_op(op, senders, receivers).await?;
                }
                self.execute_op(PatternRecognitionOp::new(&pat_cfg), senders, receivers)
                    .await
            }
            GraphNodeKind::Fusion(cfg) => {
                self.execute_op(FusionOp::new(cfg), senders, receivers)
                    .await
            }
            GraphNodeKind::Redaction(cfg) => {
                self.execute_op(RedactionOp::new(cfg), senders, receivers)
                    .await
            }
            GraphNodeKind::Validation(cfg) => {
                self.execute_op(ValidationOp::new(cfg), senders, receivers)
                    .await
            }
            GraphNodeKind::LoadContext(cfg) => {
                self.execute_op(LoadContextOp::new(cfg), senders, receivers)
                    .await
            }
            GraphNodeKind::SaveContext(cfg) => {
                self.execute_op(SaveContextOp::new(cfg), senders, receivers)
                    .await
            }
            GraphNodeKind::GenerateContext(cfg) => {
                self.execute_op(GenerateContextOp::new(cfg), senders, receivers)
                    .await
            }
            _ => Err(Error::runtime(
                format!("unsupported graph node kind: {kind}"),
                "executor",
                false,
            )),
        }
    }

    /// Generic envelope-processing loop for any [`Operation`].
    ///
    /// Receives envelopes from upstream, calls `op.execute(&mut envelope)`,
    /// and fans out to downstream senders.
    async fn execute_op(
        &self,
        op: impl Operation,
        senders: &[mpsc::Sender<DocumentEnvelope>],
        receivers: &mut [mpsc::Receiver<DocumentEnvelope>],
    ) -> Result<NodeOutput, Error> {
        let count = process_envelopes(senders, receivers, |mut envelope| {
            let op = &op;
            async move {
                op.execute(&mut envelope).await?;
                Ok(envelope)
            }
        })
        .await?;
        Ok(node_output(count))
    }

    /// Load content from the registry, decode it, and send envelopes downstream.
    async fn execute_import(
        &self,
        cfg: &ImportFile,
        retry: Option<&RetryPolicy>,
        senders: &[mpsc::Sender<DocumentEnvelope>],
    ) -> Result<NodeOutput, Error> {
        let import = ImportFileOp::new()
            .with_decompression(cfg.decompression)
            .with_decryption(cfg.decryption.clone());

        let mut count = 0u64;
        for &content_id in &cfg.content_ids {
            let shared = &self.ctx.shared;
            let import_ref = &import;
            let do_import = || async {
                tracing::debug!(target: TARGET, %content_id, "importing content");
                let handle = shared
                    .registry
                    .read_content(shared.actor_id, content_id)
                    .await?;
                let content = handle.content().await?;
                import_ref.import(content, shared).await
            };

            let envelope = match retry {
                Some(policy) => policy.with_retry(do_import).await?,
                None => do_import().await?,
            };

            fan_out(senders, envelope).await?;
            count += 1;
        }

        Ok(NodeOutput {
            items_processed: count,
            error: None,
            envelopes: Vec::new(),
        })
    }

    /// Collect processed envelopes and write them to the configured output.
    async fn execute_export(
        &self,
        cfg: &ExportFile,
        receivers: &mut [mpsc::Receiver<DocumentEnvelope>],
    ) -> Result<NodeOutput, Error> {
        let export = ExportFileOp::new()
            .with_encryption(cfg.encryption.clone())
            .with_compression(cfg.compression)
            .with_content_ids(cfg.content_ids.clone());

        let mut count = 0u64;
        let mut envelopes = Vec::new();
        for rx in receivers.iter_mut() {
            while let Some(envelope) = rx.recv().await {
                export.export(&envelope).await?;
                count += 1;
                envelopes.push(envelope);
            }
        }

        tracing::debug!(target: TARGET, count, "export complete");

        Ok(NodeOutput {
            items_processed: count,
            error: None,
            envelopes,
        })
    }
}

/// Build a successful [`NodeOutput`] with no retained envelopes.
fn node_output(items_processed: u64) -> NodeOutput {
    NodeOutput {
        items_processed,
        error: None,
        envelopes: Vec::new(),
    }
}

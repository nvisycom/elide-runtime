//! Node-level execution: operation dispatch and envelope flow.
//!
//! [`NodeExecutor`] runs a single [`ResolvedNode`] within a pipeline.
//! It matches on [`GraphNodeKind`], constructs the appropriate
//! [`Operation`], and drives the envelope processing loop:
//!
//! 1. **Receive** — pull `DocumentEnvelope` from upstream MPSC channels.
//! 2. **Call** — invoke `operation.execute(&mut envelope)`.
//! 3. **Send** — forward the envelope to all downstream channels.
//!
//! [`NodeOutput`] and [`RunOutput`] carry per-node and per-run results
//! back to the [orchestrator](super::orchestrator) and
//! [`Engine`](super::Engine) for finalization.

use std::future::Future;
use std::sync::Arc;

use futures::StreamExt;
use nvisy_codec::Document;
use nvisy_core::content::Content;
use nvisy_core::{Error, ErrorKind};
use nvisy_ontology::workflow::{
    ExportFile, GraphNode, GraphNodeKind, ImportFile, RetryPolicy, TimeoutBehavior, TimeoutPolicy,
};
use nvisy_provider::http::HttpClient;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::config::RuntimeConfig;
use super::plan::ResolvedNode;
use super::runs::RunStatus;
use crate::graph::{RetryExt, TimeoutExt};
use crate::operation::envelope::SharedData;
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
    /// If all errors are cancellation errors, returns `Cancelled`.
    /// Otherwise uses the standard ok/err breakdown.
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

/// Per-node execution context extracted from the run-level
/// [`RunContext`](super::orchestrator::RunContext).
pub(super) struct NodeContext {
    /// Shared run-wide state (run ID, actor, registry, policies, key provider).
    pub shared: Arc<SharedData>,
    /// Token to signal cancellation to this node.
    pub cancel: CancellationToken,
    /// Effective configuration after merging per-request overrides.
    pub config: Arc<RuntimeConfig>,
    /// Shared HTTP client for downstream API calls.
    pub http_client: HttpClient,
}

/// Executes a single [`ResolvedNode`] within a pipeline run.
///
/// The orchestrator creates one executor per node task.
pub(super) struct NodeExecutor {
    ctx: NodeContext,
    /// When `true`, skip post-redaction phases (validation, export).
    dry_run: bool,
}

impl NodeExecutor {
    pub fn new(ctx: NodeContext, dry_run: bool) -> Self {
        Self { ctx, dry_run }
    }

    /// Resolve the effective retry policy: node-level wins, then engine default.
    fn effective_retry(&self, node: &GraphNode) -> Option<RetryPolicy> {
        node.retry().or(self.ctx.config.default_retry()).cloned()
    }

    /// Resolve the effective timeout policy: node-level wins, then engine default.
    fn effective_timeout(&self, node: &GraphNode) -> Option<TimeoutPolicy> {
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
        if self.dry_run && resolved.node.kind.is_post_redaction() {
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
            GraphNodeKind::VisualExtraction(cfg) => {
                self.execute_op(
                    VisualExtractionOp::new(cfg, &self.ctx.config, &self.ctx.http_client)?,
                    senders,
                    receivers,
                )
                .await
            }
            GraphNodeKind::AudialExtraction(cfg) => {
                self.execute_op(
                    AudialExtractionOp::new(cfg, &self.ctx.config, &self.ctx.http_client)?,
                    senders,
                    receivers,
                )
                .await
            }
            GraphNodeKind::NamedEntityRecognition(cfg) => {
                self.execute_op(
                    EntityRecognitionOp::new(cfg, &self.ctx.config, &self.ctx.http_client).await?,
                    senders,
                    receivers,
                )
                .await
            }
            GraphNodeKind::PatternRecognition(cfg) => {
                self.execute_op(PatternRecognitionOp::new(cfg), senders, receivers)
                    .await
            }
            GraphNodeKind::Fusion(cfg) => {
                self.execute_op(FusionOp::new(cfg), senders, receivers)
                    .await
            }
            GraphNodeKind::Redaction(cfg) => {
                self.execute_op(RedactionOp::new(cfg, &self.ctx.shared), senders, receivers)
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

            let mut envelope = match retry {
                Some(policy) => policy.with_retry(do_import).await?,
                None => do_import().await?,
            };

            // Apply upload-time annotations: convert inclusions to
            // entities, store annotations for exclusion filtering.
            if !cfg.annotations.is_empty() {
                cfg.annotations
                    .apply_inclusions(&mut envelope.audit.entities);
                envelope.annotations = cfg.annotations.clone();
            }

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

/// Core envelope processing loop shared by most node types.
///
/// Merges all upstream receivers concurrently (true fan-in), applies
/// `transform` to each envelope, and fans out the result to all
/// downstream senders. Returns the total number of envelopes processed.
async fn process_envelopes<F, Fut>(
    senders: &[mpsc::Sender<DocumentEnvelope>],
    receivers: &mut [mpsc::Receiver<DocumentEnvelope>],
    mut transform: F,
) -> Result<u64, Error>
where
    F: FnMut(DocumentEnvelope) -> Fut,
    Fut: Future<Output = Result<DocumentEnvelope, Error>>,
{
    let mut count = 0u64;

    if receivers.len() <= 1 {
        // Fast path: single receiver, no merging needed.
        if let Some(rx) = receivers.first_mut() {
            while let Some(envelope) = rx.recv().await {
                let envelope = transform(envelope).await?;
                count += 1;
                fan_out(senders, envelope).await?;
            }
        }
    } else {
        // Concurrent fan-in: merge all receivers into a single stream
        // so slow upstreams don't block fast ones.
        let streams: Vec<_> = receivers
            .iter_mut()
            .map(|rx| {
                // Take ownership by swapping in a dummy closed receiver.
                let owned = {
                    let (_, mut placeholder) = mpsc::channel(1);
                    std::mem::swap(rx, &mut placeholder);
                    placeholder
                };
                Box::pin(futures::stream::unfold(owned, |mut rx| async move {
                    rx.recv().await.map(|item| (item, rx))
                }))
                    as std::pin::Pin<Box<dyn futures::Stream<Item = DocumentEnvelope> + Send>>
            })
            .collect();
        let mut merged = futures::stream::select_all(streams);

        while let Some(envelope) = StreamExt::next(&mut merged).await {
            let envelope = transform(envelope).await?;
            count += 1;
            fan_out(senders, envelope).await?;
        }
    }

    Ok(count)
}

/// Send an envelope to all downstream senders.
///
/// For fan-out (multiple senders), the envelope is cloned via
/// encode/decode to produce independent copies.
async fn fan_out(
    senders: &[mpsc::Sender<DocumentEnvelope>],
    envelope: DocumentEnvelope,
) -> Result<(), Error> {
    if senders.is_empty() {
        return Ok(());
    }

    if senders.len() == 1 {
        senders[0]
            .send(envelope)
            .await
            .map_err(|_| Error::runtime("downstream channel closed", "executor", false))?;
        return Ok(());
    }

    // Fan-out: clone the envelope for each downstream except the last.
    for tx in &senders[..senders.len() - 1] {
        let cloned = clone_envelope(&envelope).await?;
        tx.send(cloned)
            .await
            .map_err(|_| Error::runtime("downstream channel closed", "executor", false))?;
    }
    if let Some(tx) = senders.last() {
        tx.send(envelope)
            .await
            .map_err(|_| Error::runtime("downstream channel closed", "executor", false))?;
    }
    Ok(())
}

/// Drain all receivers and forward envelopes to all senders unchanged.
///
/// Used in dry-run mode to pass envelopes through skipped nodes so
/// downstream watch channels still unblock.
async fn forward_envelopes(
    senders: &[mpsc::Sender<DocumentEnvelope>],
    receivers: &mut [mpsc::Receiver<DocumentEnvelope>],
) -> Result<u64, Error> {
    let mut count = 0u64;
    for rx in receivers.iter_mut() {
        while let Some(envelope) = rx.recv().await {
            fan_out(senders, envelope).await?;
            count += 1;
        }
    }
    Ok(count)
}

/// Clone an envelope by encoding/decoding the document and cloning
/// the remaining fields.
async fn clone_envelope(envelope: &DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
    let content_data = envelope.document.encode()?;
    let content = Content::from(content_data);
    let document = Document::decode(&content).await?;
    Ok(DocumentEnvelope {
        document,
        metadata: envelope.metadata.clone(),
        annotations: envelope.annotations.clone(),
        contexts: envelope.contexts.clone(),
        audit: envelope.audit.clone(),
        shared: Arc::clone(&envelope.shared),
    })
}

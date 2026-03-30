//! Node-level execution: operation dispatch and envelope flow.
//!
//! [`NodeExecutor`] runs a single [`ResolvedNode`] within a pipeline.
//! It matches on [`GraphNodeKind`], constructs the appropriate
//! [`Operation`], and drives the envelope processing loop:
//!
//! 1. **Receive** — pull `Arc<DocumentEnvelope>` from upstream MPSC channels.
//! 2. **Extract** — unwrap ownership (cloning via encode/decode when the
//!    `Arc` refcount > 1, i.e. during fan-out).
//! 3. **Call** — invoke the operation with typed inputs.
//! 4. **Apply** — merge operation outputs back into the envelope.
//! 5. **Send** — forward the envelope to all downstream channels.
//!
//! Operations never see the [`DocumentEnvelope`] directly — they receive
//! typed inputs and produce typed outputs via the [`Operation`] trait.
//!
//! [`NodeOutput`] and [`RunOutput`] carry per-node and per-run results
//! back to the [orchestrator](super::orchestrator) and
//! [`Engine`](super::Engine) for finalization.

use std::future::Future;
use std::sync::Arc;

use futures::StreamExt;
use nvisy_codec::handler::{BoxedTextHandler, Handler, TxtHandler};
use nvisy_codec::{Document, Span};
use nvisy_core::content::Content;
use nvisy_core::{Error, ErrorKind};
use nvisy_ontology::workflow::{
    AudialExtraction, ExportFile, Fusion, GenerateContext, GraphNode, GraphNodeKind, ImportFile,
    LoadContext, NamedEntityRecognition, Redaction, RetryPolicy, SaveContext, TimeoutBehavior,
    TimeoutPolicy, Validation, VisualExtraction,
};
use nvisy_provider::http::HttpClient;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::config::RuntimeConfig;
use super::plan::ResolvedNode;
use super::runs::RunStatus;
use crate::graph::{RetryExt, TimeoutExt};
use crate::operation::context::{ParallelContext, SequentialContext, SharedContext};
use crate::operation::{
    AudialExtractionOp, AudioInput, DocumentEnvelope, EntityRecognition, ExportFileOp, FusionOp,
    GenerateContextOp, ImportFileOp, LoadContextOp, Operation, PatternRecognition, RedactionOp,
    SaveContextOp, ValidationInput, ValidationOp, VisualExtractionOp,
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
    pub envelopes: Vec<Arc<DocumentEnvelope>>,
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
    /// Shared operation context (run ID, actor, registry, policies, key provider).
    pub shared: SharedContext,
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
    ///
    /// If the node has a [`TimeoutPolicy`](nvisy_ontology::workflow::TimeoutPolicy)
    /// with [`TimeoutBehavior::Skip`](nvisy_ontology::workflow::TimeoutBehavior::Skip),
    /// a timeout produces an empty success rather than an error.
    pub async fn execute(
        &self,
        resolved: &ResolvedNode,
        senders: Vec<mpsc::Sender<Arc<DocumentEnvelope>>>,
        mut receivers: Vec<mpsc::Receiver<Arc<DocumentEnvelope>>>,
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
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        match kind {
            GraphNodeKind::ImportFile(cfg) => {
                self.execute_import(cfg, retry.as_ref(), senders).await
            }
            GraphNodeKind::ExportFile(cfg) => self.execute_export(cfg, receivers).await,
            GraphNodeKind::VisualExtraction(cfg) => {
                self.execute_visual_extraction(cfg, senders, receivers)
                    .await
            }
            GraphNodeKind::AudialExtraction(cfg) => {
                self.execute_audial_extraction(cfg, senders, receivers)
                    .await
            }
            GraphNodeKind::NamedEntityRecognition(cfg) => {
                self.execute_ner(cfg, senders, receivers).await
            }
            GraphNodeKind::PatternRecognition(cfg) => {
                self.execute_pattern_recognition(cfg, senders, receivers)
                    .await
            }
            GraphNodeKind::Fusion(cfg) => self.execute_fusion(cfg, senders, receivers).await,
            GraphNodeKind::Redaction(cfg) => self.execute_redaction(cfg, senders, receivers).await,
            GraphNodeKind::Validation(cfg) => {
                self.execute_validation(cfg, senders, receivers).await
            }
            GraphNodeKind::LoadContext(cfg) => {
                self.execute_load_context(cfg, senders, receivers).await
            }
            GraphNodeKind::SaveContext(cfg) => {
                self.execute_save_context(cfg, senders, receivers).await
            }
            GraphNodeKind::GenerateContext(cfg) => {
                self.execute_generate_context(cfg, senders, receivers).await
            }
            _ => Err(Error::runtime(
                format!("unsupported graph node kind: {kind}"),
                "executor",
                false,
            )),
        }
    }

    /// Run OCR on image spans, optionally verifying detected entities.
    async fn execute_visual_extraction(
        &self,
        cfg: &VisualExtraction,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let vis = VisualExtractionOp::new(cfg, &self.ctx.config, &self.ctx.http_client)?;
        let count = process_envelopes(senders, receivers, |mut envelope| {
            let vis = &vis;
            async move {
                tracing::debug!(target: TARGET, "extracting image spans for OCR");
                let image_spans: Vec<_> = envelope.document.collect_image_spans().await;
                if !image_spans.is_empty() {
                    let ocr_spans: Vec<Span<(), _>> = image_spans
                        .into_iter()
                        .map(|s| Span::new((), s.data).with_source(s.source))
                        .collect();

                    let _ocr_output = vis.extract(ocr_spans).await?;

                    if vis.agent().has_verifier() && !envelope.entities.is_empty() {
                        let verify_spans: Vec<_> = envelope
                            .document
                            .collect_image_spans()
                            .await
                            .into_iter()
                            .map(|s| Span::new((), s.data).with_source(s.source))
                            .collect();
                        match vis.verify(&verify_spans, envelope.entities.clone()).await {
                            Ok(output) => envelope.apply(output),
                            Err(e) => tracing::warn!(
                                target: TARGET, error = %e,
                                "OCR verification failed, keeping unverified entities"
                            ),
                        }
                    }
                }
                Ok(envelope)
            }
        })
        .await?;
        Ok(node_output(count))
    }

    /// Transcribe audio documents via STT and replace the document with text.
    async fn execute_audial_extraction(
        &self,
        cfg: &AudialExtraction,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.ctx.shared;
        let op = AudialExtractionOp::new(cfg, &self.ctx.config, &self.ctx.http_client)?;
        let count = process_envelopes(senders, receivers, |mut envelope| {
            let op = &op;
            let shared = shared.clone();
            async move {
                if let Document::Audio(ref handler) = envelope.document {
                    tracing::debug!(target: TARGET, "extracting audio for transcription");
                    let audio_data = Handler::encode(handler)?;
                    let filename = envelope
                        .metadata
                        .filename
                        .as_deref()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| "audio.wav".to_string());
                    let input = ParallelContext::new(
                        AudioInput {
                            audio_data: audio_data.as_bytes().to_vec(),
                            filename,
                        },
                        shared,
                    );
                    let output = op.call(input).await?;
                    let stt_result = output.into_inner();
                    if !stt_result.text.is_empty() {
                        let lines: Vec<String> =
                            stt_result.text.lines().map(String::from).collect();
                        let trailing = stt_result.text.ends_with('\n');
                        let source = envelope.document.source();
                        let handler =
                            TxtHandler::new(lines, trailing).with_source(source);
                        envelope.document =
                            Document::from(BoxedTextHandler::from(handler));
                        tracing::debug!(target: TARGET, "replaced audio document with transcript text");
                    }
                }
                Ok(envelope)
            }
        })
        .await?;
        Ok(node_output(count))
    }

    /// Run named entity recognition on text spans via the LLM agent.
    async fn execute_ner(
        &self,
        cfg: &NamedEntityRecognition,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.ctx.shared;
        let op = EntityRecognition::new(cfg, &self.ctx.config, &self.ctx.http_client).await?;
        let count = process_envelopes(senders, receivers, |mut envelope| {
            let op = &op;
            let shared = shared.clone();
            async move {
                let spans: Vec<_> = envelope.document.collect_text_spans().await;
                if !spans.is_empty() {
                    tracing::debug!(target: TARGET, span_count = spans.len(), "running NER");
                    let input = SequentialContext::new(spans, shared);
                    let output = op.call(input).await?;
                    envelope.apply(output.into_inner());
                }
                op.reset().await;
                Ok(envelope)
            }
        })
        .await?;
        Ok(node_output(count))
    }

    /// Run regex and dictionary pattern recognition on text spans.
    async fn execute_pattern_recognition(
        &self,
        cfg: &nvisy_ontology::workflow::PatternRecognition,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.ctx.shared;
        let op = PatternRecognition::new(cfg);
        let count = process_envelopes(senders, receivers, |mut envelope| {
            let op = &op;
            let shared = shared.clone();
            async move {
                let spans: Vec<_> = envelope.document.collect_text_spans().await;
                if !spans.is_empty() {
                    let input = ParallelContext::new(spans, shared);
                    let output = op.call(input).await?;
                    envelope.apply(output.into_inner());
                }
                Ok(envelope)
            }
        })
        .await?;
        Ok(node_output(count))
    }

    /// Merge overlapping or adjacent entities from multiple detection sources.
    async fn execute_fusion(
        &self,
        cfg: &Fusion,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.ctx.shared;
        let op = FusionOp::new(cfg);
        let count = process_envelopes(senders, receivers, |mut envelope| {
            let op = &op;
            let shared = shared.clone();
            async move {
                if !envelope.entities.is_empty() {
                    tracing::debug!(target: TARGET, entities = envelope.entities.len(), "running fusion");
                    let input = ParallelContext::new(envelope.entities.clone(), shared);
                    let output = op.call(input).await?;
                    envelope.apply(output.into_inner());
                }
                Ok(envelope)
            }
        }).await?;
        Ok(node_output(count))
    }

    /// Evaluate redaction policies against detected entities.
    async fn execute_redaction(
        &self,
        cfg: &Redaction,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.ctx.shared;
        let op = RedactionOp::new(cfg, shared).await?;
        let count = process_envelopes(senders, receivers, |mut envelope| {
            let op = &op;
            let shared = shared.clone();
            async move {
                if !envelope.entities.is_empty() {
                    tracing::debug!(target: TARGET, entities = envelope.entities.len(), "evaluating redaction policies");
                    let input = ParallelContext::new(envelope.entities.clone(), shared);
                    let output = op.call(input).await?;
                    envelope.apply(output.into_inner());
                }
                Ok(envelope)
            }
        }).await?;
        Ok(node_output(count))
    }

    /// Validate redaction decisions against the final document state.
    async fn execute_validation(
        &self,
        cfg: &Validation,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.ctx.shared;
        let op = ValidationOp::new(cfg);
        let count = process_envelopes(senders, receivers, |envelope| {
            let op = &op;
            let shared = shared.clone();
            async move {
                let text_spans: Vec<_> = envelope.document.collect_text_spans().await;
                let redacted_text = if text_spans.is_empty() {
                    None
                } else {
                    Some(
                        text_spans
                            .iter()
                            .map(|s| s.data.as_str())
                            .collect::<String>(),
                    )
                };
                let input = ParallelContext::new(
                    ValidationInput {
                        entities: envelope.entities.clone(),
                        decisions: envelope.audit.decisions.clone(),
                        redacted_text,
                    },
                    shared,
                );
                op.call(input).await?;
                Ok(envelope)
            }
        })
        .await?;
        Ok(node_output(count))
    }

    /// Attach pre-loaded context references to each envelope.
    async fn execute_load_context(
        &self,
        cfg: &LoadContext,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.ctx.shared;
        let op = LoadContextOp::new(cfg);
        let count = process_envelopes(senders, receivers, |mut envelope| {
            let op = &op;
            let shared = shared.clone();
            async move {
                tracing::debug!(target: TARGET, "adding context references to envelope");
                let input = ParallelContext::new(envelope.contexts.clone(), shared);
                let output = op.call(input).await?;
                envelope.contexts = output.data;
                Ok(envelope)
            }
        })
        .await?;
        Ok(node_output(count))
    }

    /// Persist envelope contexts back to the registry.
    async fn execute_save_context(
        &self,
        cfg: &SaveContext,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.ctx.shared;
        let op = SaveContextOp::new(cfg);
        let count = process_envelopes(senders, receivers, |envelope| {
            let op = &op;
            let shared = shared.clone();
            async move {
                tracing::debug!(target: TARGET, "saving contexts to registry");
                let input = ParallelContext::new(envelope.contexts.clone(), shared);
                op.call(input).await?;
                Ok(envelope)
            }
        })
        .await?;
        Ok(node_output(count))
    }

    /// Generate new context entries (currently a passthrough).
    async fn execute_generate_context(
        &self,
        cfg: &GenerateContext,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.ctx.shared;
        let op = GenerateContextOp::new(cfg);
        let count = process_envelopes(senders, receivers, |envelope| {
            let op = &op;
            let shared = shared.clone();
            async move {
                tracing::debug!(target: TARGET, "generate context passthrough");
                let input = ParallelContext::new((), shared);
                op.call(input).await?;
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
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let import = ImportFileOp::new()
            .with_decompression(cfg.decompression)
            .with_decryption(cfg.decryption.clone());

        let mut count = 0u64;
        for &content_id in &cfg.content_ids {
            let registry = &self.ctx.shared.registry;
            let actor_id = self.ctx.shared.actor_id;
            let import_ref = &import;
            let do_import = || async {
                tracing::debug!(target: TARGET, %content_id, "importing content");
                let handle = registry.read_content(actor_id, content_id).await?;
                let content = handle.content().await?;
                let input = ParallelContext::new(content, self.ctx.shared.clone());
                let output = import_ref.call(input).await?;
                Ok(output.into_inner())
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
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let export = ExportFileOp::new()
            .with_encryption(cfg.encryption.clone())
            .with_compression(cfg.compression)
            .with_content_ids(cfg.content_ids.clone());

        let mut count = 0u64;
        let mut envelopes = Vec::new();
        for rx in receivers.iter_mut() {
            while let Some(envelope) = rx.recv().await {
                let owned = unwrap_envelope(envelope).await?;
                let input = ParallelContext::new(owned, self.ctx.shared.clone());
                let output = export.call(input).await?;
                count += 1;
                envelopes.push(Arc::new(output.into_inner()));
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
    senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
    receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
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
            while let Some(item) = rx.recv().await {
                let envelope = unwrap_envelope(item).await?;
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
                let (_, mut dummy) = mpsc::channel(1);
                std::mem::swap(rx, &mut dummy);
                Box::pin(futures::stream::unfold(dummy, |mut rx| async move {
                    rx.recv().await.map(|item| (item, rx))
                }))
                    as std::pin::Pin<Box<dyn futures::Stream<Item = Arc<DocumentEnvelope>> + Send>>
            })
            .collect();
        let mut merged = futures::stream::select_all(streams);

        while let Some(item) = StreamExt::next(&mut merged).await {
            let envelope = unwrap_envelope(item).await?;
            let envelope = transform(envelope).await?;
            count += 1;
            fan_out(senders, envelope).await?;
        }
    }

    Ok(count)
}

/// Send an envelope to all downstream senders.
///
/// Clones the `Arc` for all senders except the last, which receives
/// ownership to avoid an unnecessary refcount bump.
async fn fan_out(
    senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
    envelope: DocumentEnvelope,
) -> Result<(), Error> {
    if senders.is_empty() {
        return Ok(());
    }
    let envelope = Arc::new(envelope);
    for tx in &senders[..senders.len() - 1] {
        tx.send(Arc::clone(&envelope))
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
    senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
    receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
) -> Result<u64, Error> {
    let mut count = 0u64;
    for rx in receivers.iter_mut() {
        while let Some(envelope) = rx.recv().await {
            for tx in senders {
                tx.send(Arc::clone(&envelope))
                    .await
                    .map_err(|_| Error::runtime("downstream channel closed", "executor", false))?;
            }
            count += 1;
        }
    }
    Ok(count)
}

/// Take ownership of an envelope from an `Arc`.
///
/// When the refcount is 1, this is a zero-cost unwrap. During fan-out
/// (refcount > 1), the document is cloned via encode/decode to produce
/// an independent copy while entity, context, and audit data are cloned
/// directly.
async fn unwrap_envelope(arc: Arc<DocumentEnvelope>) -> Result<DocumentEnvelope, Error> {
    match Arc::try_unwrap(arc) {
        Ok(envelope) => Ok(envelope),
        Err(arc) => {
            let content_data = arc.document.encode()?;
            let content = Content::from(content_data);
            let document = Document::decode(&content).await?;
            Ok(DocumentEnvelope {
                document,
                metadata: arc.metadata.clone(),
                entities: arc.entities.clone(),
                contexts: arc.contexts.clone(),
                audit: arc.audit.clone(),
            })
        }
    }
}

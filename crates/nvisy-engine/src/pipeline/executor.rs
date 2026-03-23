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

use std::sync::Arc;

use futures::StreamExt;
use nvisy_codec::handler::{BoxedTextHandler, Handler, TxtHandler};
use nvisy_codec::{Document, Span};
use nvisy_core::{Error, ErrorKind};
use nvisy_http::HttpClient;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::config::RuntimeConfig;
use super::plan::ResolvedNode;
use super::runs::RunStatus;
use crate::graph::{self, GraphNodeKind, RetryPolicy, TimeoutBehavior};
use crate::operation::context::{ParallelContext, SequentialContext, SharedContext};
use crate::operation::{
    AudialExtraction, AudioInput, DocumentEnvelope, EntityRecognition, ExportFile, Fusion,
    GenerateContext, ImportFile, LoadContext, Operation, PatternRecognition, Redaction,
    SaveContext, Validation, ValidationInput, VerifyInput, VisualExtraction,
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
    pub fn run_status(&self) -> RunStatus {
        let any_ok = self.node_results.iter().any(|r| r.error.is_none());
        let any_err = self.node_results.iter().any(|r| r.error.is_some());
        match (any_ok, any_err) {
            (_, false) => RunStatus::Succeeded,
            (true, true) => RunStatus::PartialFailure,
            _ => RunStatus::Failed,
        }
    }
}

/// Executes a single [`ResolvedNode`] within a pipeline run.
///
/// Owns clones of the run-scoped context, cancellation token, config,
/// and HTTP client. The orchestrator creates one executor per node task.
pub(super) struct NodeExecutor {
    shared: SharedContext,
    cancel: CancellationToken,
    config: RuntimeConfig,
    http_client: HttpClient,
}

impl NodeExecutor {
    pub fn new(
        shared: SharedContext,
        cancel: CancellationToken,
        config: RuntimeConfig,
        http_client: HttpClient,
    ) -> Self {
        Self {
            shared,
            cancel,
            config,
            http_client,
        }
    }

    /// Execute a resolved node, applying timeout and cancellation policies.
    ///
    /// If the node has a [`TimeoutPolicy`](crate::graph::TimeoutPolicy)
    /// with [`TimeoutBehavior::Skip`](crate::graph::TimeoutBehavior::Skip),
    /// a timeout produces an empty success rather than an error.
    pub async fn execute(
        &self,
        resolved: &ResolvedNode,
        senders: Vec<mpsc::Sender<Arc<DocumentEnvelope>>>,
        mut receivers: Vec<mpsc::Receiver<Arc<DocumentEnvelope>>>,
    ) -> Result<NodeOutput, Error> {
        if self.cancel.is_cancelled() {
            return Err(Error::cancellation("run cancelled"));
        }

        let cancel = self.cancel.clone();

        let run = async {
            tokio::select! {
                _ = cancel.cancelled() => Err(Error::cancellation("run cancelled")),
                result = self.dispatch(
                    &resolved.node.kind,
                    resolved.retry.clone(),
                    &senders,
                    &mut receivers,
                ) => result,
            }
        };

        match &resolved.timeout {
            Some(timeout) => {
                let result: Result<NodeOutput, Error> = timeout.with_timeout(run).await;
                match (&result, &timeout.on_timeout) {
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
            GraphNodeKind::PatternRecognition(_) => {
                self.execute_pattern_recognition(senders, receivers).await
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
        }
    }

    /// Run OCR on image spans, optionally verifying detected entities.
    async fn execute_visual_extraction(
        &self,
        cfg: &graph::VisualExtraction,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.shared;
        let op = VisualExtraction::new(cfg, &self.config, &self.http_client)?;
        let count = process_envelopes(senders, receivers, |mut envelope| {
            let op = &op;
            let shared = shared.clone();
            async move {
                tracing::debug!(target: TARGET, "extracting image spans for OCR");
                let image_spans: Vec<_> = envelope.document.image_spans().await.collect().await;
                if !image_spans.is_empty() {
                    let ocr_spans: Vec<Span<(), _>> = image_spans
                        .into_iter()
                        .map(|s| Span::new((), s.data).with_source(s.source))
                        .collect();

                    let input = ParallelContext::new(ocr_spans, shared.clone());
                    let _ocr_output = op.ocr().call(input).await?;

                    if let Some(verifier) = op.verifier()
                        && !envelope.entities.is_empty()
                    {
                        let verify_spans: Vec<_> = envelope
                            .document
                            .image_spans()
                            .await
                            .collect::<Vec<_>>()
                            .await
                            .into_iter()
                            .map(|s| Span::new((), s.data).with_source(s.source))
                            .collect();
                        let verify_input = VerifyInput {
                            image_spans: verify_spans,
                            entities: envelope.entities.clone(),
                        };
                        let input = ParallelContext::new(verify_input, shared.clone());
                        match verifier.call(input).await {
                            Ok(output) => envelope.apply(output.into_inner()),
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
        cfg: &graph::AudialExtraction,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.shared;
        let op = AudialExtraction::new(cfg, &self.config, &self.http_client)?;
        let count = process_envelopes(senders, receivers, |mut envelope| {
            let op = &op;
            let shared = shared.clone();
            async move {
                if let Document::Audio(ref handler) = envelope.document {
                    tracing::debug!(target: TARGET, "extracting audio for transcription");
                    let audio_data = Handler::encode(handler)?;
                    let filename: String = audio_data
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
        cfg: &graph::NamedEntityRecognition,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.shared;
        let op = EntityRecognition::new(cfg, &self.config, &self.http_client).await?;
        let count = process_envelopes(senders, receivers, |mut envelope| {
            let op = &op;
            let shared = shared.clone();
            async move {
                let spans: Vec<_> = envelope.document.text_spans().await.collect().await;
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

    /// Run regex-based pattern recognition on text spans.
    async fn execute_pattern_recognition(
        &self,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.shared;
        let op = PatternRecognition::new().await?;
        let count = process_envelopes(senders, receivers, |mut envelope| {
            let op = &op;
            let shared = shared.clone();
            async move {
                let spans: Vec<_> = envelope.document.text_spans().await.collect().await;
                if !spans.is_empty() {
                    tracing::debug!(target: TARGET, span_count = spans.len(), "running pattern detection");
                    let input = ParallelContext::new(spans, shared);
                    let output = op.call(input).await?;
                    envelope.apply(output.into_inner());
                }
                Ok(envelope)
            }
        }).await?;
        Ok(node_output(count))
    }

    /// Merge overlapping or adjacent entities from multiple detection sources.
    async fn execute_fusion(
        &self,
        cfg: &graph::Fusion,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.shared;
        let op = Fusion::new(cfg);
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
        cfg: &graph::Redaction,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.shared;
        let op = Redaction::new(cfg, shared).await?;
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
        cfg: &graph::Validation,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.shared;
        let op = Validation::new(cfg);
        let count = process_envelopes(senders, receivers, |envelope| {
            let op = &op;
            let shared = shared.clone();
            async move {
                let text_spans: Vec<_> = envelope.document.text_spans().await.collect().await;
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
        cfg: &graph::LoadContext,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.shared;
        let op = LoadContext::new(cfg);
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
        cfg: &graph::SaveContext,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.shared;
        let op = SaveContext::new(cfg);
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
        cfg: &graph::GenerateContext,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.shared;
        let op = GenerateContext::new(cfg);
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
        cfg: &graph::ImportFile,
        retry: Option<&RetryPolicy>,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let import = ImportFile::new()
            .with_decompression(cfg.decompression)
            .with_decryption(cfg.decryption.clone());

        let mut count = 0u64;
        for &content_id in &cfg.content_ids {
            let registry = &self.shared.registry;
            let actor_id = self.shared.actor_id;
            let import_ref = &import;
            let do_import = || async {
                tracing::debug!(target: TARGET, %content_id, "importing content");
                let handle = registry.read_content(actor_id, content_id).await?;
                let content_data = handle.content_data().await?;
                let input = ParallelContext::new(content_data, self.shared.clone());
                let output = import_ref.call(input).await?;
                Ok(output.into_inner())
            };

            let envelope = match retry {
                Some(policy) => policy.with_retry(do_import).await?,
                None => do_import().await?,
            };

            let envelope = Arc::new(envelope);
            if senders.len() == 1 {
                senders[0]
                    .send(envelope)
                    .await
                    .map_err(|_| Error::runtime("downstream channel closed", "executor", false))?;
            } else {
                for tx in &senders[..senders.len() - 1] {
                    tx.send(Arc::clone(&envelope)).await.map_err(|_| {
                        Error::runtime("downstream channel closed", "executor", false)
                    })?;
                }
                if let Some(tx) = senders.last() {
                    tx.send(envelope).await.map_err(|_| {
                        Error::runtime("downstream channel closed", "executor", false)
                    })?;
                }
            }
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
        cfg: &graph::ExportFile,
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let export = ExportFile::new()
            .with_encryption(cfg.encryption.clone())
            .with_compression(cfg.compression)
            .with_content_ids(cfg.content_ids.clone());

        let mut count = 0u64;
        let mut envelopes = Vec::new();
        for rx in receivers.iter_mut() {
            while let Some(envelope) = rx.recv().await {
                let owned = unwrap_envelope(envelope).await?;
                let input = ParallelContext::new(owned, self.shared.clone());
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
/// Drains all upstream receivers, applies `transform` to each envelope,
/// and fans out the result to all downstream senders. Returns the total
/// number of envelopes processed.
async fn process_envelopes<F, Fut>(
    senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
    receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    mut transform: F,
) -> Result<u64, Error>
where
    F: FnMut(DocumentEnvelope) -> Fut,
    Fut: std::future::Future<Output = Result<DocumentEnvelope, Error>>,
{
    let mut count = 0u64;
    for rx in receivers.iter_mut() {
        while let Some(item) = rx.recv().await {
            let envelope = unwrap_envelope(item).await?;
            let envelope = transform(envelope).await?;

            count += 1;
            let envelope = Arc::new(envelope);
            if senders.len() == 1 {
                senders[0]
                    .send(envelope)
                    .await
                    .map_err(|_| Error::runtime("downstream channel closed", "executor", false))?;
            } else {
                for tx in &senders[..senders.len() - 1] {
                    tx.send(Arc::clone(&envelope)).await.map_err(|_| {
                        Error::runtime("downstream channel closed", "executor", false)
                    })?;
                }
                if let Some(tx) = senders.last() {
                    tx.send(envelope).await.map_err(|_| {
                        Error::runtime("downstream channel closed", "executor", false)
                    })?;
                }
            }
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
            let document = Document::decode(&content_data).await?;
            Ok(DocumentEnvelope {
                document,
                entities: arc.entities.clone(),
                contexts: arc.contexts.clone(),
                audit: arc.audit.clone(),
            })
        }
    }
}

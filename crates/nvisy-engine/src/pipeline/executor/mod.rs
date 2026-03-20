//! Node-level execution: dispatch and envelope flow.
//!
//! The executor matches on [`GraphNodeKind`], constructs the appropriate
//! operation, then runs the standard recv → extract → call → apply → send
//! loop. Operations never see the [`DocumentEnvelope`] — they receive
//! typed inputs and produce typed outputs via the [`Operation`] trait.

use std::sync::Arc;

use futures::StreamExt;
use nvisy_codec::handler::{Handler, ImageHandler, TextData, TextHandler};
use nvisy_codec::{Document, Span};
use nvisy_core::{Error, ErrorKind};
use nvisy_http::HttpClient;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::config::RuntimeConfig;
use super::plan::ResolvedNode;
use crate::graph::{self, GraphNodeKind, RetryPolicy, TimeoutBehavior};
use crate::operation::context::{ParallelContext, SequentialContext, SharedContext};
use crate::operation::{
    AudioInput, DocumentEnvelope, EntityRecognition, Fusion, GenerateContext, ImportFile,
    LoadContext, Operation, PatternRecognition, Redaction, SaveContext, Validation,
    ValidationInput, VerifyInput, VisualExtraction,
};

const TARGET: &str = "nvisy_engine::pipeline::executor";

/// Outcome of executing a single node.
#[derive(Debug)]
pub(super) struct NodeOutput {
    #[allow(dead_code)]
    pub node_id: Uuid,
    pub items_processed: u64,
    pub error: Option<String>,
    pub envelopes: Vec<Arc<DocumentEnvelope>>,
}

/// Aggregate outcome of executing the full graph.
#[derive(Debug)]
pub(super) struct RunOutput {
    pub node_results: Vec<NodeOutput>,
}

/// Executes a single resolved node within a pipeline run.
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

    /// Execute a resolved node, applying timeout and cancellation.
    pub async fn execute(
        &self,
        resolved: &ResolvedNode,
        senders: Vec<mpsc::Sender<Arc<DocumentEnvelope>>>,
        mut receivers: Vec<mpsc::Receiver<Arc<DocumentEnvelope>>>,
    ) -> Result<NodeOutput, Error> {
        if self.cancel.is_cancelled() {
            return Err(Error::cancellation("run cancelled"));
        }

        let node_id = resolved.node.id;
        let cancel = self.cancel.clone();

        let run = async {
            tokio::select! {
                _ = cancel.cancelled() => Err(Error::cancellation("run cancelled")),
                result = self.dispatch(
                    node_id,
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
                            node_id,
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

    #[allow(clippy::too_many_lines)]
    async fn dispatch(
        &self,
        node_id: Uuid,
        kind: &GraphNodeKind,
        retry: Option<RetryPolicy>,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.shared;

        match kind {
            GraphNodeKind::ImportFile(cfg) => {
                self.execute_import(node_id, cfg, retry.as_ref(), senders)
                    .await
            }
            GraphNodeKind::ExportFile(cfg) => {
                self.execute_export(node_id, cfg, receivers).await
            }

            GraphNodeKind::VisualExtraction(cfg) => {
                let op = VisualExtraction::new(
                    cfg, &self.config, &self.http_client, shared.clone(), retry.clone(),
                )?;
                let count = process_envelopes(senders, receivers, |mut envelope| {
                    let op = &op;
                    let shared = shared.clone();
                    async move {
                        if let Document::Image(ref handler) = envelope.document {
                            tracing::debug!(target: TARGET, "extracting image spans for OCR");
                            let image_spans: Vec<_> = handler.image_spans().await.collect().await;
                            let ocr_spans: Vec<Span<(), _>> = image_spans
                                .into_iter()
                                .map(|s| Span::new((), s.data).with_source(s.source))
                                .collect();

                            let input = ParallelContext::new(ocr_spans, shared.clone());
                            let _ocr_output = op.ocr().call(input).await?;

                            if let Some(verifier) = op.verifier() {
                                if !envelope.entities.is_empty() {
                                    let verify_spans: Vec<_> = match &envelope.document {
                                        Document::Image(h) => h
                                            .image_spans().await
                                            .collect::<Vec<_>>().await
                                            .into_iter()
                                            .map(|s| Span::new((), s.data).with_source(s.source))
                                            .collect(),
                                        _ => Vec::new(),
                                    };
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
                        }
                        Ok(envelope)
                    }
                }).await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::AudialExtraction(cfg) => {
                let op = crate::operation::AudialExtraction::new(
                    cfg, &self.config, &self.http_client, retry.clone(),
                )?;
                let count = process_envelopes(senders, receivers, |envelope| {
                    let op = &op;
                    let shared = shared.clone();
                    async move {
                        if let Document::Audio(ref handler) = envelope.document {
                            tracing::debug!(target: TARGET, "extracting audio for transcription");
                            let audio_data = Handler::encode(handler)?;
                            let filename: String = audio_data
                                .filename.as_deref()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|| "audio.wav".to_string());

                            let input = ParallelContext::new(
                                AudioInput { audio_data: audio_data.as_bytes().to_vec(), filename },
                                shared,
                            );
                            let _output = op.call(input).await?;
                            // TODO: inject transcribed text into envelope
                        }
                        Ok(envelope)
                    }
                }).await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::NamedEntityRecognition(cfg) => {
                let op = EntityRecognition::new(
                    cfg, &self.config, &self.http_client, shared.clone(), retry.clone(),
                ).await?;
                let count = process_envelopes(senders, receivers, |mut envelope| {
                    let op = &op;
                    let shared = shared.clone();
                    async move {
                        let spans = collect_text_spans(&envelope.document).await;
                        if !spans.is_empty() {
                            tracing::debug!(target: TARGET, span_count = spans.len(), "running NER");
                            let input = SequentialContext::new(spans, shared);
                            let output = op.call(input).await?;
                            envelope.apply(output.into_inner());
                        }
                        op.reset().await;
                        Ok(envelope)
                    }
                }).await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::PatternRecognition(_) => {
                let op = PatternRecognition::new(shared.clone()).await?;
                let count = process_envelopes(senders, receivers, |mut envelope| {
                    let op = &op;
                    let shared = shared.clone();
                    async move {
                        let spans = collect_text_spans(&envelope.document).await;
                        if !spans.is_empty() {
                            tracing::debug!(target: TARGET, span_count = spans.len(), "running pattern detection");
                            let input = ParallelContext::new(spans, shared);
                            let output = op.call(input).await?;
                            envelope.apply(output.into_inner());
                        }
                        Ok(envelope)
                    }
                }).await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::Fusion(cfg) => {
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
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::Redaction(cfg) => {
                let op = Redaction::new(cfg, shared.clone(), retry.clone()).await?;
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
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::Validation(cfg) => {
                let op = Validation::new(cfg);
                let count = process_envelopes(senders, receivers, |envelope| {
                    let op = &op;
                    let shared = shared.clone();
                    async move {
                        tracing::debug!(target: TARGET, "running post-redaction validation");
                        let redacted_text = match &envelope.document {
                            Document::Text(h) => {
                                let spans: Vec<_> = h.text_spans().await.collect().await;
                                Some(spans.iter().map(|s| s.data.as_str()).collect::<String>())
                            }
                            _ => None,
                        };
                        let input = ParallelContext::new(
                            ValidationInput {
                                entities: envelope.entities.clone(),
                                decisions: envelope.audit.decisions.clone(),
                                redacted_text,
                            },
                            shared,
                        );
                        let output = op.call(input).await?;
                        let result = output.data;
                        if !result.leaked.is_empty() {
                            tracing::warn!(
                                target: TARGET,
                                leaked = result.leaked.len(),
                                passed = result.passed,
                                "validation found leaked values",
                            );
                            if cfg.fail_on_leak {
                                return Err(Error::validation(
                                    format!("{} redacted values leaked in output", result.leaked.len()),
                                    "validation",
                                ));
                            }
                        } else {
                            tracing::debug!(target: TARGET, passed = result.passed, "validation passed");
                        }
                        Ok(envelope)
                    }
                }).await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::LoadContext(cfg) => {
                let op = LoadContext::new(cfg, shared).await?;
                let count = process_envelopes(senders, receivers, |mut envelope| {
                    let op = &op;
                    let shared = shared.clone();
                    async move {
                        tracing::debug!(target: TARGET, "merging loaded contexts into envelope");
                        let input = ParallelContext::new(envelope.contexts.clone(), shared);
                        let output = op.call(input).await?;
                        envelope.contexts = output.data;
                        Ok(envelope)
                    }
                }).await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::SaveContext(cfg) => {
                let op = SaveContext::new(cfg, shared);
                let count = process_envelopes(senders, receivers, |envelope| {
                    let op = &op;
                    let shared = shared.clone();
                    async move {
                        tracing::debug!(target: TARGET, "saving contexts to registry");
                        let input = ParallelContext::new(envelope.contexts.clone(), shared);
                        op.call(input).await?;
                        Ok(envelope)
                    }
                }).await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::GenerateContext(cfg) => {
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
                }).await?;
                Ok(node_output(node_id, count))
            }
        }
    }

    async fn execute_import(
        &self,
        node_id: Uuid,
        cfg: &graph::ImportFile,
        retry: Option<&RetryPolicy>,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let import = ImportFile::new()
            .with_decompression(cfg.decompression)
            .with_decryption(cfg.decryption);

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
            for tx in senders {
                let _ = tx.send(Arc::clone(&envelope)).await;
            }
            count += 1;
        }

        Ok(NodeOutput {
            node_id,
            items_processed: count,
            error: None,
            envelopes: Vec::new(),
        })
    }

    async fn execute_export(
        &self,
        node_id: Uuid,
        _cfg: &graph::ExportFile,
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let mut count = 0u64;
        let mut envelopes = Vec::new();
        for rx in receivers.iter_mut() {
            while let Some(envelope) = rx.recv().await {
                count += 1;
                envelopes.push(envelope);
            }
        }

        tracing::debug!(target: TARGET, count, "export collected envelopes");

        Ok(NodeOutput {
            node_id,
            items_processed: count,
            error: None,
            envelopes,
        })
    }
}

fn node_output(node_id: Uuid, items_processed: u64) -> NodeOutput {
    NodeOutput {
        node_id,
        items_processed,
        error: None,
        envelopes: Vec::new(),
    }
}

/// Receive envelopes from upstream, transform, send downstream.
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
            for tx in senders {
                let _ = tx.send(Arc::clone(&envelope)).await;
            }
        }
    }
    Ok(count)
}

/// Take ownership of an envelope from an `Arc`, cloning via
/// encode/decode when the reference count is > 1 (fan-out).
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

/// Collect text spans from text or rich documents for NER/pattern detection.
async fn collect_text_spans(doc: &Document) -> Vec<Span<usize, TextData>> {
    match doc {
        Document::Text(h) => h.text_spans().await.collect().await,
        Document::Rich(h) => h.text_spans().await.collect().await,
        _ => Vec::new(),
    }
}

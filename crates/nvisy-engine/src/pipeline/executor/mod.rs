//! Node-level execution: dispatch and envelope flow.
//!
//! The executor matches on [`GraphNodeKind`], constructs the appropriate
//! operation, then runs the standard recv → extract → call → apply → send
//! loop. Operations never see the [`DocumentEnvelope`] — they receive
//! typed inputs and produce typed outputs via the [`Operation`] trait.

use std::sync::Arc;

use futures::StreamExt;
use nvisy_codec::handler::{ImageHandler, TextHandler};
use nvisy_codec::{Document, Span};
use nvisy_core::{Error, ErrorKind};
use nvisy_http::HttpClient;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::config::RuntimeConfig;
use super::plan::ResolvedNode;
use crate::graph::{self, GraphNodeKind, RetryPolicy, TimeoutBehavior};
use crate::operation::{
    DocumentEnvelope, Fusion, ImportFile, Operation, ParallelContext, SharedContext,
    VisualExtraction,
};

const TARGET: &str = "nvisy_engine::pipeline::executor";

/// Outcome of executing a single node.
#[derive(Debug)]
pub(super) struct NodeOutput {
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

    async fn dispatch(
        &self,
        node_id: Uuid,
        kind: &GraphNodeKind,
        retry: Option<RetryPolicy>,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let shared = &self.shared;
        let retry_ref = retry.as_ref();

        match kind {
            GraphNodeKind::ImportFile(cfg) => {
                self.execute_import(node_id, cfg, retry_ref, senders).await
            }
            GraphNodeKind::ExportFile(cfg) => self.execute_export(node_id, cfg, receivers).await,

            GraphNodeKind::VisualExtraction(cfg) => {
                let op = VisualExtraction::connect(
                    cfg,
                    &self.config,
                    &self.http_client,
                    shared.clone(),
                    retry.clone(),
                )?;
                let count = process_envelopes(senders, receivers, |mut envelope| async {
                    // OCR extracts from images — the operation handles extraction internally
                    // the operation handles extraction internally
                    // once the OCR output can be applied as a patch.

                    envelope = op.process(envelope).await?;
                    Ok(envelope)
                })
                .await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::AudialExtraction(cfg) => {
                let op = crate::operation::AudialExtraction::connect(
                    cfg,
                    &self.config,
                    &self.http_client,
                    retry.clone(),
                )?;
                let count = process_envelopes(senders, receivers, |envelope| async {
                    op.process(envelope).await
                })
                .await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::NamedEntityRecognition(cfg) => {
                let op = crate::operation::EntityRecognition::connect(
                    cfg,
                    &self.config,
                    &self.http_client,
                    shared.clone(),
                    retry.clone(),
                )
                .await?;
                let count = process_envelopes(senders, receivers, |envelope| async {
                    op.process(envelope).await
                })
                .await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::PatternRecognition(_) => {
                let op = crate::operation::PatternRecognition::connect(shared.clone()).await?;
                let count = process_envelopes(senders, receivers, |mut envelope| async {
                    envelope = op.process(envelope).await?;
                    Ok(envelope)
                })
                .await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::Fusion(cfg) => {
                let op = Fusion::from_graph(cfg);
                let count = process_envelopes(senders, receivers, |mut envelope| {
                    let op_ref = &op;
                    async move {
                        if !envelope.entities.is_empty() {
                            let result = op_ref.execute(envelope.entities.clone()).await?;
                            envelope.apply(result);
                        }
                        Ok(envelope)
                    }
                })
                .await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::Redaction(cfg) => {
                let op = crate::operation::Redaction::connect(cfg, shared.clone(), retry.clone())
                    .await?;
                let count = process_envelopes(senders, receivers, |mut envelope| {
                    let op_ref = &op;
                    async move {
                        if !envelope.entities.is_empty() {
                            let outcome = op_ref.evaluate(envelope.entities.clone()).await?;
                            envelope.apply(outcome);
                        }
                        Ok(envelope)
                    }
                })
                .await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::Validation(cfg) => {
                let op = crate::operation::Validation::new(cfg);
                let count = process_envelopes(senders, receivers, |envelope| async {
                    op.process(envelope).await
                })
                .await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::LoadContext(cfg) => {
                let mut loaded = Vec::with_capacity(cfg.context_ids.len());
                for &id in &cfg.context_ids {
                    let handle = shared.registry.read_context(shared.actor_id, id).await?;
                    loaded.push(handle.context().await?);
                }
                let count = process_envelopes(senders, receivers, |mut envelope| {
                    let loaded = &loaded;
                    async move {
                        for ctx in loaded {
                            let id = ctx.source.as_uuid();
                            if !envelope.contexts.contains(&id) {
                                envelope.contexts.insert(ctx.clone());
                            }
                        }
                        Ok(envelope)
                    }
                })
                .await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::SaveContext(cfg) => {
                let registry = shared.registry.clone();
                let actor_id = shared.actor_id;
                let context_ids = cfg.context_ids.clone();
                let count = process_envelopes(senders, receivers, |envelope| {
                    let registry = &registry;
                    let context_ids = &context_ids;
                    async move {
                        for &id in context_ids.iter() {
                            if let Some(context) = envelope.contexts.get(&id) {
                                registry.register_context(actor_id, context.clone()).await?;
                            }
                        }
                        Ok(envelope)
                    }
                })
                .await?;
                Ok(node_output(node_id, count))
            }

            GraphNodeKind::GenerateContext(_cfg) => {
                // Stub — passes through unchanged
                let count =
                    process_envelopes(senders, receivers, |envelope| async { Ok(envelope) })
                        .await?;
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

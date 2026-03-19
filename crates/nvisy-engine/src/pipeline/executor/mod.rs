//! Node-level execution: dispatch and envelope flow.
//!
//! [`NodeExecutor`] builds operations from [`GraphNodeKind`] and runs
//! the standard recv → transform → send loop. Import and Export are
//! structural exceptions handled directly.

use std::sync::Arc;

use nvisy_codec::Document;
use nvisy_core::{Error, ErrorKind};
use nvisy_http::HttpClient;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::config::RuntimeConfig;
use super::plan::ResolvedNode;
use crate::graph::{self, GraphNodeKind, RetryPolicy, TimeoutBehavior};
use crate::operation::{
    AudialExtraction, DocumentEnvelope, EntityRecognition, Fusion, GenerateContext, ImportFile,
    LoadContext, NodeHandler, Operation, ParallelContext, PatternRecognition, Redaction,
    SaveContext, SharedContext, Validation, VisualExtraction,
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
            Some(compiled) => {
                let result: Result<NodeOutput, Error> = compiled.with_timeout(run).await;
                match (&result, &compiled.on_timeout) {
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
        match kind {
            GraphNodeKind::Import(cfg) => {
                self.execute_import(node_id, cfg, retry.as_ref(), senders)
                    .await
            }
            GraphNodeKind::Export(cfg) => self.execute_export(node_id, cfg, receivers).await,
            _ => {
                let handler = self.build_operation(kind, retry).await?;
                let count =
                    process_envelopes(senders, receivers, |envelope| handler.handle(envelope))
                        .await?;
                Ok(NodeOutput {
                    node_id,
                    items_processed: count,
                    error: None,
                    envelopes: Vec::new(),
                })
            }
        }
    }

    async fn build_operation(
        &self,
        kind: &GraphNodeKind,
        retry: Option<RetryPolicy>,
    ) -> Result<Box<dyn NodeHandler>, Error> {
        match kind {
            GraphNodeKind::VisualExtraction(cfg) => Ok(Box::new(VisualExtraction::connect(
                cfg,
                &self.config,
                &self.http_client,
                self.shared.clone(),
                retry,
            )?)),
            GraphNodeKind::AudialExtraction(cfg) => Ok(Box::new(AudialExtraction::connect(
                cfg,
                &self.config,
                &self.http_client,
                retry,
            )?)),
            GraphNodeKind::NamedEntityRecognition(cfg) => Ok(Box::new(
                EntityRecognition::connect(
                    cfg,
                    &self.config,
                    &self.http_client,
                    self.shared.clone(),
                    retry,
                )
                .await?,
            )),
            GraphNodeKind::PatternRecognition(_) => Ok(Box::new(
                PatternRecognition::connect(self.shared.clone()).await?,
            )),
            GraphNodeKind::Fusion(cfg) => Ok(Box::new(Fusion::from_graph(cfg))),
            GraphNodeKind::Redaction(cfg) => Ok(Box::new(
                Redaction::connect(cfg, self.shared.clone(), retry).await?,
            )),
            GraphNodeKind::Validation(cfg) => Ok(Box::new(Validation::new(cfg))),
            GraphNodeKind::LoadContext(cfg) => {
                Ok(Box::new(LoadContext::connect(cfg, &self.shared).await?))
            }
            GraphNodeKind::SaveContext(cfg) => Ok(Box::new(SaveContext::new(cfg, &self.shared))),
            GraphNodeKind::GenerateContext(cfg) => Ok(Box::new(GenerateContext::new(cfg))),
            GraphNodeKind::Import(_) | GraphNodeKind::Export(_) => {
                unreachable!("import/export handled directly in dispatch")
            }
        }
    }

    async fn execute_import(
        &self,
        node_id: Uuid,
        cfg: &graph::Import,
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
        _cfg: &graph::Export,
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

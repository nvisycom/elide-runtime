//! Node-level execution: dispatch, envelope flow, and handler construction.
//!
//! [`NodeExecutor`] builds a [`NodeHandler`] from the node's [`GraphNodeKind`]
//! and config, then runs the standard recv → transform → send loop via
//! [`process_envelopes`]. Import and Export are structural exceptions
//! handled directly.

mod handler;
mod handlers;

use std::sync::Arc;

use nvisy_codec::Document;
use nvisy_core::{Error, ErrorKind};
use nvisy_http::HttpClient;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use self::handler::NodeHandler;
use self::handlers::context::{GenerateContextHandler, LoadContextHandler, SaveContextHandler};
use self::handlers::extraction::{AudialExtractionHandler, VisualExtractionHandler};
use self::handlers::recognition::{NerHandler, PatternRecognitionHandler};
use self::handlers::refinement::{FusionHandler, RedactionHandler, ValidationHandler};
use super::config::RuntimeConfig;
use super::plan::ResolvedNode;
use super::policy::CompiledRetryPolicy;
use crate::graph::{self, GraphNodeKind};
use crate::graph::policy::TimeoutBehavior;
use crate::operation::lifecycle::ImportFile;
use crate::operation::{DocumentEnvelope, Operation, ParallelContext, SharedContext};

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
        Self { shared, cancel, config, http_client }
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
                    resolved.compiled_retry.clone(),
                    &senders,
                    &mut receivers,
                ) => result,
            }
        };

        match &resolved.compiled_timeout {
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

    /// Dispatch to the appropriate handler based on node kind.
    async fn dispatch(
        &self,
        node_id: Uuid,
        kind: &GraphNodeKind,
        retry: Option<CompiledRetryPolicy>,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        match kind {
            GraphNodeKind::Import(cfg) => {
                self.execute_import(node_id, cfg, retry.as_ref(), senders).await
            }
            GraphNodeKind::Export(cfg) => {
                self.execute_export(node_id, cfg, receivers).await
            }
            _ => {
                let handler = self.build_handler(kind, retry).await?;
                let count = process_envelopes(senders, receivers, |envelope| {
                    handler.handle(envelope)
                })
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

    /// Build a [`NodeHandler`] for a transform node.
    async fn build_handler(
        &self,
        kind: &GraphNodeKind,
        retry: Option<CompiledRetryPolicy>,
    ) -> Result<Box<dyn NodeHandler>, Error> {
        match kind {
            GraphNodeKind::VisualExtraction(cfg) => Ok(Box::new(
                VisualExtractionHandler::new(cfg, &self.config, &self.http_client, self.shared.clone(), retry)?,
            )),
            GraphNodeKind::AudialExtraction(cfg) => Ok(Box::new(
                AudialExtractionHandler::new(cfg, &self.config, &self.http_client, retry)?,
            )),
            GraphNodeKind::NamedEntityRecognition(cfg) => Ok(Box::new(
                NerHandler::new(cfg, &self.config, &self.http_client, self.shared.clone(), retry).await?,
            )),
            GraphNodeKind::PatternRecognition(_) => Ok(Box::new(
                PatternRecognitionHandler::new(self.shared.clone(), retry).await?,
            )),
            GraphNodeKind::Fusion(cfg) => Ok(Box::new(
                FusionHandler::new(cfg, self.shared.clone(), retry),
            )),
            GraphNodeKind::Redaction(cfg) => Ok(Box::new(
                RedactionHandler::new(cfg, self.shared.clone(), retry).await?,
            )),
            GraphNodeKind::Validation(cfg) => Ok(Box::new(
                ValidationHandler::new(cfg, self.shared.clone()),
            )),
            GraphNodeKind::LoadContext(cfg) => Ok(Box::new(
                LoadContextHandler::new(cfg, self.shared.clone()).await?,
            )),
            GraphNodeKind::SaveContext(cfg) => Ok(Box::new(
                SaveContextHandler::new(cfg, self.shared.clone()),
            )),
            GraphNodeKind::GenerateContext(cfg) => Ok(Box::new(
                GenerateContextHandler::new(cfg),
            )),
            GraphNodeKind::Import(_) | GraphNodeKind::Export(_) => {
                unreachable!("import/export handled directly in dispatch")
            }
        }
    }

    async fn execute_import(
        &self,
        node_id: Uuid,
        cfg: &graph::Import,
        retry: Option<&CompiledRetryPolicy>,
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
        // Export post-processing (encryption, compression)
        // will be applied when ExportFile gains real implementation.
        // For now, export just collects envelopes for the engine output.

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

/// Receive envelopes from upstream, transform each one, and send downstream.
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

/// Take ownership of the envelope from an `Arc`, cloning via
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

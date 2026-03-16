//! Node-level execution dispatchers.
//!
//! [`NodeExecutor`] dispatches each graph node to the appropriate handler
//! based on its [`GraphNodeKind`]. Pre-compiled timeout and retry policies
//! from the [`ResolvedNode`] are applied directly, with
//! [`TimeoutBehavior`] controlling whether a timeout is treated as an error
//! or silently yields zero items.

use std::sync::Arc;

use nvisy_core::{Error, ErrorKind};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::plan::ResolvedNode;
use crate::graph::GraphNodeKind;
use crate::graph::policy::TimeoutBehavior;
use crate::operation::{DocumentEnvelope, SharedContext};

/// Outcome of executing a single node in the pipeline.
#[derive(Debug)]
pub(super) struct NodeOutput {
    /// ID of the node that produced this result.
    pub node_id: Uuid,
    /// Number of data items processed by this node.
    pub items_processed: u64,
    /// Error message if the node failed, or `None` on success.
    pub error: Option<String>,
    /// Envelopes collected by terminal (export) nodes.
    pub envelopes: Vec<Arc<DocumentEnvelope>>,
}

/// Aggregate outcome of executing an entire pipeline graph.
#[derive(Debug)]
pub(super) struct RunOutput {
    /// Per-node results in completion order.
    pub node_results: Vec<NodeOutput>,
}

/// Executes a single resolved node within a pipeline run.
pub(super) struct NodeExecutor {
    shared: SharedContext,
    cancel: CancellationToken,
}

impl NodeExecutor {
    pub fn new(shared: SharedContext, cancel: CancellationToken) -> Self {
        Self { shared, cancel }
    }

    /// Execute a resolved node, applying timeout policies and cancellation.
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
                _ = cancel.cancelled() => {
                    Err(Error::cancellation("run cancelled"))
                }
                result = self.dispatch(node_id, &resolved.node.kind, &senders, &mut receivers) => {
                    result
                }
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

    /// Dispatch based on node kind: Import decodes content, Export collects
    /// envelopes, all others pass through.
    async fn dispatch(
        &self,
        node_id: Uuid,
        action: &GraphNodeKind,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        match action {
            GraphNodeKind::Import(_) => self.execute_import(node_id, senders, receivers).await,
            GraphNodeKind::Export(_) => self.execute_export(node_id, receivers).await,
            kind => {
                self.execute_passthrough(node_id, kind, senders, receivers)
                    .await
            }
        }
    }

    async fn execute_import(
        &self,
        node_id: Uuid,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        // TODO: Import nodes should receive ContentData from the registry,
        // not from upstream channels. For now this is a stub that will be
        // wired when content storage is connected.
        let mut count = 0u64;
        let mut envelopes = Vec::new();

        for rx in receivers.iter_mut() {
            while let Some(envelope) = rx.recv().await {
                for tx in senders {
                    let _ = tx.send(Arc::clone(&envelope)).await;
                }
                count += 1;
                envelopes.push(envelope);
            }
        }

        Ok(NodeOutput {
            node_id,
            items_processed: count,
            error: None,
            envelopes,
        })
    }

    async fn execute_export(
        &self,
        node_id: Uuid,
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

        tracing::debug!(count, "export node collected envelopes");

        Ok(NodeOutput {
            node_id,
            items_processed: count,
            error: None,
            envelopes,
        })
    }

    async fn execute_passthrough(
        &self,
        node_id: Uuid,
        kind: &GraphNodeKind,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let label = match kind {
            GraphNodeKind::LoadContext(_) => "load_context",
            GraphNodeKind::SaveContext(_) => "save_context",
            GraphNodeKind::GenerateContext(_) => "generate_context",
            GraphNodeKind::VisualExtraction(_) => "visual_extraction",
            GraphNodeKind::AudialExtraction(_) => "audial_extraction",
            GraphNodeKind::NamedEntityRecognition(_) => "ner",
            GraphNodeKind::PatternRecognition(_) => "pattern_recognition",
            GraphNodeKind::Fusion(_) => "fusion",
            GraphNodeKind::Redaction(_) => "redaction",
            GraphNodeKind::Import(_) | GraphNodeKind::Export(_) => unreachable!(),
        };

        // TODO: wire operation dispatch
        tracing::trace!(action = label, "passthrough");

        let mut count = 0u64;
        for rx in receivers.iter_mut() {
            while let Some(item) = rx.recv().await {
                count += 1;
                for tx in senders {
                    let _ = tx.send(Arc::clone(&item)).await;
                }
            }
        }

        Ok(NodeOutput {
            node_id,
            items_processed: count,
            error: None,
            envelopes: Vec::new(),
        })
    }
}

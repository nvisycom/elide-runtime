//! Context handlers: load, save, and generate.

use nvisy_core::Error;

use super::super::handler::NodeHandler;
use crate::graph;
use crate::operation::lifecycle;
use crate::operation::{DocumentEnvelope, Operation, ParallelContext, SharedContext};

const TARGET: &str = "nvisy_engine::pipeline::executor";

pub(crate) struct LoadContextHandler {
    op: lifecycle::LoadContext,
    shared: SharedContext,
}

impl LoadContextHandler {
    pub async fn new(
        cfg: &graph::LoadContext,
        shared: SharedContext,
    ) -> Result<Self, Error> {
        let op = lifecycle::LoadContext::connect(
            &shared.registry,
            shared.actor_id,
            &cfg.context_ids,
        )
        .await?;
        Ok(Self { op, shared })
    }
}

#[async_trait::async_trait]
impl NodeHandler for LoadContextHandler {
    async fn handle(&self, mut envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        let input = ParallelContext::new(envelope.contexts.clone(), self.shared.clone());
        let output = self.op.call(input).await?;
        envelope.contexts = output.data;
        Ok(envelope)
    }
}

pub(crate) struct SaveContextHandler {
    op: lifecycle::SaveContext,
    shared: SharedContext,
}

impl SaveContextHandler {
    pub fn new(cfg: &graph::SaveContext, shared: SharedContext) -> Self {
        let op = lifecycle::SaveContext::new(
            shared.actor_id,
            cfg.context_ids.clone(),
            shared.registry.clone(),
        );
        Self { op, shared }
    }
}

#[async_trait::async_trait]
impl NodeHandler for SaveContextHandler {
    async fn handle(&self, envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        let input = ParallelContext::new(envelope.contexts.clone(), self.shared.clone());
        self.op.call(input).await?;
        Ok(envelope)
    }
}

pub(crate) struct GenerateContextHandler;

impl GenerateContextHandler {
    pub fn new(cfg: &graph::GenerateContext) -> Self {
        if cfg.summarization {
            tracing::warn!(target: TARGET, "summarization not yet implemented, skipping");
        }
        if cfg.translation {
            tracing::warn!(target: TARGET, "translation not yet implemented, skipping");
        }
        if cfg.audit {
            tracing::debug!(target: TARGET, "audit records already accumulated on envelope");
        }
        Self
    }
}

#[async_trait::async_trait]
impl NodeHandler for GenerateContextHandler {
    async fn handle(&self, envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        Ok(envelope)
    }
}

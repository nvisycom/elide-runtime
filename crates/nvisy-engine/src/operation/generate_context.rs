//! Generate contexts from pipeline results (stub).

use nvisy_core::{Error, Result};

use crate::operation::{DocumentEnvelope, NodeHandler};

const TARGET: &str = "nvisy_engine::op::generate_context";

/// Generates contexts from detection results and content data.
/// Currently a stub — will support summarization, translation,
/// and audit context generation.
pub struct GenerateContext;

impl GenerateContext {
    pub fn new(cfg: &crate::graph::GenerateContext) -> Self {
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
impl NodeHandler for GenerateContext {
    async fn handle(&self, envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        Ok(envelope)
    }
}

//! Generate context operation (stub).
//!
//! Runs at **phase 4** alongside [`Redaction`]. Will eventually support
//! summarization, translation, and audit context generation.
//!
//! [`Redaction`]: crate::operation::Redaction

use nvisy_core::{Error, Result};

use crate::graph::GenerateContext as GenerateContextCfg;
use crate::operation::{DocumentEnvelope, Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::generate_context";

/// Generates contexts from detection results and content data.
///
/// Currently a passthrough stub.
pub struct GenerateContext;

impl GenerateContext {
    /// Create from graph config.
    pub fn new(cfg: &GenerateContextCfg) -> Self {
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

    pub(crate) async fn process(
        &self,
        envelope: DocumentEnvelope,
    ) -> Result<DocumentEnvelope, Error> {
        tracing::debug!(target: TARGET, "generate context passthrough");
        Ok(envelope)
    }
}

impl Default for GenerateContext {
    fn default() -> Self {
        Self
    }
}

impl Operation for GenerateContext {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|_| async { Ok(()) }).await
    }
}

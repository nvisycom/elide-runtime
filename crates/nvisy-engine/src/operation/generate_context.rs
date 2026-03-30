//! Generate context operation (stub).
//!
//! Runs at **phase 4** alongside [`Redaction`]. Will eventually support
//! summarization, translation, and audit context generation.
//!
//! [`Redaction`]: crate::operation::Redaction

use nvisy_core::Result;
use nvisy_ontology::workflow::GenerateContext;

use crate::operation::Operation;
use crate::operation::context::ParallelContext;

const TARGET: &str = "nvisy_engine::op::generate_context";

/// Generates contexts from detection results and content data.
///
/// Currently a passthrough stub.
pub struct GenerateContextOp;

impl GenerateContextOp {
    /// Create from graph config.
    pub fn new(cfg: &GenerateContext) -> Self {
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

impl Default for GenerateContextOp {
    fn default() -> Self {
        Self
    }
}

impl Operation for GenerateContextOp {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|_| async { Ok(()) }).await
    }
}

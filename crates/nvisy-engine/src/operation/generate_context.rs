//! Generate context operation (stub).
//!
//! Runs at **phase 4** alongside [`Redaction`]. Will eventually support
//! summarization, translation, and audit context generation.
//!
//! [`Redaction`]: crate::operation::RedactionOp

use nvisy_core::Result;
use nvisy_ontology::workflow::GenerateContext;

use crate::operation::{DocumentEnvelope, Operation};

/// Generates contexts from detection results and content data.
///
/// Currently a passthrough stub.
pub struct GenerateContextOp;

impl GenerateContextOp {
    /// Create from graph config.
    pub fn new(cfg: &GenerateContext) -> Self {
        if cfg.summarization {
            tracing::warn!("summarization not yet implemented, skipping");
        }
        if cfg.translation {
            tracing::warn!("translation not yet implemented, skipping");
        }
        if cfg.audit {
            tracing::debug!("audit records already accumulated on envelope");
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
    async fn execute(&self, _envelope: &mut DocumentEnvelope) -> Result<()> {
        Ok(())
    }
}

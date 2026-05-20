//! Generate context operation (stub).
//!
//! Runs at **phase 4** alongside [`Redaction`]. Will eventually support
//! summarization, translation, and audit context generation.
//!
//! [`Redaction`]: crate::operation::Redaction

use nvisy_core::Result;
use nvisy_ontology::workflow::GenerateContext as GenerateContextConfig;

use crate::operation::{DocumentEnvelope, Operation};

/// Generates contexts from detection results and content data.
///
/// Currently a passthrough stub.
pub struct GenerateContext;

impl GenerateContext {
    /// Create from graph config.
    pub fn new(cfg: &GenerateContextConfig) -> Self {
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

impl Default for GenerateContext {
    fn default() -> Self {
        Self
    }
}

impl Operation for GenerateContext {
    async fn execute(&self, _envelope: &mut DocumentEnvelope) -> Result<()> {
        Ok(())
    }
}

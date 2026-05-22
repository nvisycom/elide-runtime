//! Generate context operation (stub).
//!
//! Runs at **phase 4** alongside [`Redaction`]. Will eventually support
//! summarization, translation, and audit context generation.
//!
//! [`Redaction`]: crate::operation::Redaction

use nvisy_core::Result;

use crate::context::GenerateContext as GenerateContextConfig;
use crate::operation::DocumentEnvelope;

/// Generates contexts from detection results and content data.
///
/// Currently a passthrough stub.
pub struct ContextGenerator;

impl ContextGenerator {
    /// Create from config.
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

    /// Execute the operation (currently a passthrough).
    pub async fn execute(&self, _envelope: &mut DocumentEnvelope) -> Result<()> {
        Ok(())
    }
}

impl Default for ContextGenerator {
    fn default() -> Self {
        Self
    }
}

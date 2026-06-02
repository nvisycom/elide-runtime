//! [`NoopBackend`]: zero-entities backend. The default.
//!
//! Used by tests and by deployments that detect via patterns / LLM
//! only and don't want NER at all.

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_core::nlp::RawNerSpan;

use super::gliner_backend::{GlinerBackend, GlinerRequest};

/// Zero-entities backend.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopBackend;

impl NoopBackend {
    /// Construct a [`NoopBackend`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl GlinerBackend for NoopBackend {
    async fn predict(&self, _request: GlinerRequest<'_>) -> Result<Vec<RawNerSpan>> {
        Ok(Vec::new())
    }
}

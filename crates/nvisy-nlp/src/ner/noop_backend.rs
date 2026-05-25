//! [`NoopBackend`] — returns no entities. Default selection in
//! [`NlpDetection`], and the placeholder backend for tests and
//! pipelines where NER is opt-in and the caller chose to opt out.
//!
//! [`NlpDetection`]: ../../../nvisy_engine/detection/nlp/struct.NlpDetection.html

use async_trait::async_trait;
use nvisy_ontology::entity::Entities;

use super::{NerBackend, NerParams};
use crate::error::Result;

/// A [`NerBackend`] that produces no entities.
///
/// Useful in unit tests that need an `Engine` without a real
/// model, and as a placeholder in pipelines where NER is opt-in and
/// the caller chose to opt out.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopBackend;

impl NoopBackend {
    /// Construct an empty backend.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NerBackend for NoopBackend {
    async fn recognize(&self, _text: &str, _params: NerParams<'_>) -> Result<Entities> {
        Ok(Entities::new())
    }
}

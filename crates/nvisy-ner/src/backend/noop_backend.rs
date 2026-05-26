//! [`NoopBackend`] — returns no entities. Default selection in
//! [`NerDetection`], and the placeholder backend for tests and
//! pipelines where NER is opt-in and the caller chose to opt out.
//!
//! [`NerDetection`]: ../../../nvisy_engine/detection/ner/struct.NerDetection.html

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_ontology::entity::Entities;
use nvisy_ontology::modality::Text;

use crate::core::{Backend, Context};

/// A [`Backend`] that produces no entities.
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
impl Backend for NoopBackend {
    async fn recognize(&self, _text: &str, _ctx: &Context) -> Result<Entities<Text>> {
        Ok(Entities::new())
    }
}

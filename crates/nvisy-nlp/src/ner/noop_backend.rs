//! [`NoopBackend`] — returns no entities. Used by the
//! [`NlpPreset::Default`] preset and as a placeholder when a pipeline
//! is wired through patterns or LLM only, with NER deliberately
//! off.
//!
//! [`Engine`]: crate::engine::Engine
//! [`NlpPreset::Default`]: crate::preset::NlpPreset::Default

use async_trait::async_trait;
use nvisy_ontology::entity::{Entities, EntityKind};
use nvisy_ontology::primitive::LanguageTag;

use super::NerBackend;
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
    async fn recognize(
        &self,
        _text: &str,
        _language: Option<&LanguageTag>,
        _requested_kinds: Option<&[EntityKind]>,
    ) -> Result<Entities> {
        Ok(Entities::new())
    }
}

//! [`NoopNerBackend`] — returns no entities. Intended for tests and
//! for assembling an [`NlpEngine`] when only tokenization or language
//! detection is needed.
//!
//! [`NlpEngine`]: crate::engine::NlpEngine

use async_trait::async_trait;
use nvisy_ontology::entity::Entities;
use nvisy_ontology::primitive::LanguageTag;

use super::NerBackend;
use crate::error::NlpError;

/// A [`NerBackend`] that produces no entities.
///
/// Useful in unit tests that need an `NlpEngine` without a real
/// model, and as a placeholder in pipelines where NER is opt-in and
/// the caller chose to opt out.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopNerBackend;

impl NoopNerBackend {
    /// Construct an empty backend.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NerBackend for NoopNerBackend {
    async fn recognize(
        &self,
        _text: &str,
        _language: Option<&LanguageTag>,
    ) -> Result<Entities, NlpError> {
        Ok(Entities::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_no_entities() {
        let backend = NoopNerBackend::new();
        let entities = backend.recognize("anything", None).await.unwrap();
        assert!(entities.is_empty());
    }
}

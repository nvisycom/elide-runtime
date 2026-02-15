//! AI-powered named-entity recognition (NER) detection action.

use serde::Deserialize;

use nvisy_ingest::document::Document;
use nvisy_ingest::handler::TxtHandler;
use nvisy_ontology::entity::Entity;
use nvisy_core::error::Error;

#[cfg(feature = "image-redaction")]
use nvisy_ingest::handler::PngHandler;

use crate::action::Action;

fn default_confidence() -> f64 {
    0.5
}

/// Typed parameters for [`DetectNerAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectNerParams {
    /// Entity types to detect (empty = all).
    #[serde(default)]
    pub entity_types: Vec<String>,
    /// Minimum confidence score for returned entities.
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f64,
}

/// Typed input for [`DetectNerAction`].
pub struct DetectNerInput {
    /// Text documents to scan for named entities.
    pub text_docs: Vec<Document<TxtHandler>>,
    /// Image documents to scan for named entities (feature-gated).
    #[cfg(feature = "image-redaction")]
    pub image_docs: Vec<Document<PngHandler>>,
}

/// AI NER detection stub — delegates to an NER model provider at runtime.
pub struct DetectNerAction;

#[async_trait::async_trait]
impl Action for DetectNerAction {
    type Params = DetectNerParams;
    type Input = DetectNerInput;
    type Output = Vec<Entity>;

    fn id(&self) -> &str {
        "detect-ner"
    }

    async fn connect(_params: Self::Params) -> Result<Self, Error> {
        Ok(Self)
    }

    async fn execute(
        &self,
        _input: Self::Input,
    ) -> Result<Vec<Entity>, Error> {
        // Stub: real implementation will call an NER model provider.
        Ok(Vec::new())
    }
}

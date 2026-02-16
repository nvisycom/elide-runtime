//! OCR text extraction action — generates text entities with bounding boxes
//! from image documents.

use serde::Deserialize;

use nvisy_codec::document::Document;
use nvisy_codec::handler::{PngHandler, TxtHandler};
use nvisy_ontology::entity::Entity;
use nvisy_core::error::Error;

use crate::action::Action;

fn default_language() -> String {
    "eng".into()
}

fn default_engine() -> String {
    "tesseract".into()
}

fn default_confidence() -> f64 {
    0.5
}

/// Typed parameters for [`GenerateOcrAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateOcrParams {
    /// OCR language code (ISO 639-3).
    #[serde(default = "default_language")]
    pub language: String,
    /// OCR engine identifier.
    #[serde(default = "default_engine")]
    pub engine: String,
    /// Minimum confidence score for returned entities.
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f64,
}

/// Typed input for [`GenerateOcrAction`].
pub struct GenerateOcrInput {
    /// Image documents to extract text from.
    pub image_docs: Vec<Document<PngHandler>>,
}

/// Typed output for [`GenerateOcrAction`].
pub struct GenerateOcrOutput {
    /// Detected text entities with bounding-box locations.
    pub entities: Vec<Entity>,
    /// Extracted text as new text documents.
    pub text_docs: Vec<Document<TxtHandler>>,
}

/// OCR generation stub — delegates to an OCR engine provider at runtime.
pub struct GenerateOcrAction;

#[async_trait::async_trait]
impl Action for GenerateOcrAction {
    type Params = GenerateOcrParams;
    type Input = GenerateOcrInput;
    type Output = GenerateOcrOutput;

    fn id(&self) -> &str {
        "generate-ocr"
    }

    async fn connect(_params: Self::Params) -> Result<Self, Error> {
        Ok(Self)
    }

    async fn execute(
        &self,
        _input: Self::Input,
    ) -> Result<GenerateOcrOutput, Error> {
        // Stub: real implementation will call an OCR engine provider.
        Ok(GenerateOcrOutput {
            entities: Vec::new(),
            text_docs: Vec::new(),
        })
    }
}

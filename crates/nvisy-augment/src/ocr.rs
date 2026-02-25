//! OCR text extraction action — generates text entities with bounding boxes
//! from image documents.

use serde::Deserialize;

use nvisy_codec::document::Document;
use nvisy_codec::handler::{Handler, PngHandler, TxtHandler};
use nvisy_core::Error;

use nvisy_ontology::entity::Entity;

pub use nvisy_rig::paddle::{OcrBackend, OcrConfig, parse_ocr_entities};

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

/// OCR generation — delegates to an [`OcrBackend`] at runtime.
pub struct GenerateOcrAction<B> {
    backend: B,
    params: GenerateOcrParams,
}

impl<B: OcrBackend> GenerateOcrAction<B> {
    /// Create a new action with the given backend and params.
    pub fn new(backend: B, params: GenerateOcrParams) -> Self {
        Self { backend, params }
    }

    /// Build the [`OcrConfig`] from action parameters.
    fn config(&self) -> OcrConfig {
        OcrConfig {
            language: self.params.language.clone(),
            engine: self.params.engine.clone(),
            confidence_threshold: self.params.confidence_threshold,
        }
    }

    /// Execute OCR on image documents.
    pub async fn run(&self, input: GenerateOcrInput) -> Result<GenerateOcrOutput, Error> {
        let config = self.config();
        let mut all_entities = Vec::new();
        let mut all_ocr_text = Vec::new();

        for doc in &input.image_docs {
            let png_bytes = doc.handler().encode()?;
            let raw = self
                .backend
                .detect_ocr(&png_bytes, "image/png", &config)
                .await?;
            let entities = parse_ocr_entities(&raw)?;
            for entity in &entities {
                all_ocr_text.push(entity.value.clone());
            }
            all_entities.extend(entities);
        }

        let mut text_docs = Vec::new();
        if !all_ocr_text.is_empty() {
            let text = all_ocr_text.join("\n");
            let handler = TxtHandler::new(
                text.lines().map(String::from).collect(),
                text.ends_with('\n'),
            );
            text_docs.push(Document::new(handler));
        }

        Ok(GenerateOcrOutput {
            entities: all_entities,
            text_docs,
        })
    }
}

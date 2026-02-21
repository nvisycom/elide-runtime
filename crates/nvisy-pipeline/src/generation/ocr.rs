//! OCR text extraction action — generates text entities with bounding boxes
//! from image documents.

use serde::Deserialize;
use serde_json::Value;

use nvisy_codec::document::Document;
use nvisy_codec::handler::{Handler, PngHandler, TxtHandler};
use nvisy_core::data::EntityCategory;
use nvisy_core::Error;
use nvisy_core::math::BoundingBox;

use crate::ontology::{DetectionMethod, Entity, ImageLocation};

fn default_language() -> String {
    "eng".into()
}

fn default_engine() -> String {
    "tesseract".into()
}

fn default_confidence() -> f64 {
    0.5
}

/// Configuration passed to an [`OcrBackend`] implementation.
#[derive(Debug, Clone)]
pub struct OcrConfig {
    /// Language hint (e.g. `"eng"` for English).
    pub language: String,
    /// OCR engine to use (`"tesseract"`, `"google-vision"`, `"aws-textract"`).
    pub engine: String,
    /// Minimum confidence threshold for OCR results.
    pub confidence_threshold: f64,
}

/// Backend trait for OCR providers.
///
/// Implementations call an external OCR service and return raw JSON
/// results.  Entity construction is handled by [`GenerateOcrAction`].
#[async_trait::async_trait]
pub trait OcrBackend: Send + Sync + 'static {
    /// Run OCR on image bytes, returning raw dicts.
    async fn detect_ocr(
        &self,
        image_data: &[u8],
        mime_type: &str,
        config: &OcrConfig,
    ) -> Result<Vec<Value>, Error>;
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

/// Parse raw JSON dicts from an OCR backend into [`Entity`] values.
///
/// Expected dict keys: `text`, `x`, `y`, `width`, `height`, `confidence`.
pub fn parse_ocr_entities(raw: &[Value]) -> Result<Vec<Entity>, Error> {
    let mut entities = Vec::new();

    for item in raw {
        let obj = item.as_object().ok_or_else(|| {
            Error::python("Expected JSON object in OCR results".to_string())
        })?;

        let text = obj
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::python("Missing 'text' in OCR result".to_string()))?;

        let x = obj.get("x").and_then(Value::as_f64).unwrap_or(0.0);
        let y = obj.get("y").and_then(Value::as_f64).unwrap_or(0.0);
        let width = obj.get("width").and_then(Value::as_f64).unwrap_or(0.0);
        let height = obj.get("height").and_then(Value::as_f64).unwrap_or(0.0);
        let confidence = obj.get("confidence").and_then(Value::as_f64).unwrap_or(0.0);

        let entity = Entity::new(
            EntityCategory::Pii,
            "ocr_text",
            text,
            DetectionMethod::Ocr,
            confidence,
        )
        .with_image_location(ImageLocation {
            bounding_box: BoundingBox { x, y, width, height },
            image_id: None,
            page_number: None,
        });

        entities.push(entity);
    }

    Ok(entities)
}

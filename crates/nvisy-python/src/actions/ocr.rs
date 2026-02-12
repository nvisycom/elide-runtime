//! OCR detection pipeline action.

use serde::Deserialize;

use nvisy_ingest::handler::{FormatHandler, PlaintextHandler};
use nvisy_ingest::document::Document;
use nvisy_ontology::ontology::entity::Entity;
use nvisy_core::error::Error;
use nvisy_core::io::ContentData;
use nvisy_pipeline::action::Action;
use crate::bridge::PythonBridge;
use crate::ocr::{self, OcrConfig};

/// Typed parameters for [`OcrDetectAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrDetectParams {
    /// Language hint (default `"eng"`).
    #[serde(default = "default_language")]
    pub language: String,
    /// OCR engine to use.
    #[serde(default = "default_engine")]
    pub engine: String,
    /// Minimum confidence threshold.
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f64,
}

fn default_language() -> String {
    "eng".to_string()
}
fn default_engine() -> String {
    "tesseract".to_string()
}
fn default_confidence() -> f64 {
    0.5
}

/// Pipeline action that performs OCR on images and produces entities
/// with bounding boxes, plus `Document` artifacts from concatenated
/// OCR text so downstream regex/dictionary/NER can process it.
pub struct OcrDetectAction {
    /// Python bridge used to call the OCR backend.
    pub bridge: PythonBridge,
    params: OcrDetectParams,
}

impl OcrDetectAction {
    /// Replace the default bridge with a pre-configured one.
    pub fn with_bridge(mut self, bridge: PythonBridge) -> Self {
        self.bridge = bridge;
        self
    }
}

#[async_trait::async_trait]
impl Action for OcrDetectAction {
    type Params = OcrDetectParams;
    type Input = (ContentData, Vec<Document<FormatHandler>>);
    type Output = (Vec<Entity>, Vec<Document<FormatHandler>>);

    fn id(&self) -> &str {
        "detect-ocr"
    }

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        Ok(Self { bridge: PythonBridge::default(), params })
    }

    async fn execute(
        &self,
        input: Self::Input,
    ) -> Result<Self::Output, Error> {
        let (content, images) = input;
        let config = OcrConfig {
            language: self.params.language.clone(),
            engine: self.params.engine.clone(),
            confidence_threshold: self.params.confidence_threshold,
        };

        let mut all_entities = Vec::new();
        let mut all_ocr_text = Vec::new();

        if images.is_empty() {
            // Treat content as a single image
            let mime_type = content
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string();
            let entities =
                ocr::detect_ocr(&self.bridge, content.as_bytes(), &mime_type, &config).await?;
            for entity in &entities {
                all_ocr_text.push(entity.value.clone());
            }
            all_entities.extend(entities);
        } else {
            for doc in &images {
                if let (Some(data), Some(mime)) = (&doc.data, &doc.mime_type) {
                    let entities =
                        ocr::detect_ocr(&self.bridge, data, mime, &config)
                            .await?;
                    for entity in &entities {
                        all_ocr_text.push(entity.value.clone());
                    }
                    all_entities.extend(entities);
                }
            }
        }

        // Create a Document from concatenated OCR text for downstream processing
        let mut documents = Vec::new();
        if !all_ocr_text.is_empty() {
            let ocr_doc = Document::new(FormatHandler::Plaintext(PlaintextHandler)).with_text(all_ocr_text.join("\n"));
            documents.push(ocr_doc);
        }

        Ok((all_entities, documents))
    }
}

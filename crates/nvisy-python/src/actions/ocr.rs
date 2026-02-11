//! OCR detection pipeline action.

use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::{Document, ImageData};
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::registry::action::Action;
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
/// with bounding boxes, plus a `Document` artifact from concatenated
/// OCR text so downstream regex/dictionary/NER can process it.
pub struct OcrDetectAction {
    /// Python bridge used to call the OCR backend.
    pub bridge: PythonBridge,
}

#[async_trait::async_trait]
impl Action for OcrDetectAction {
    type Params = OcrDetectParams;

    fn id(&self) -> &str {
        "detect-ocr"
    }

    fn validate_params(&self, _params: &Self::Params) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        params: Self::Params,
    ) -> Result<u64, Error> {
        let config = OcrConfig {
            language: params.language,
            engine: params.engine,
            confidence_threshold: params.confidence_threshold,
        };
        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            let images: Vec<ImageData> = blob.get_artifacts("images").map_err(|e| {
                Error::new(
                    ErrorKind::Runtime,
                    format!("failed to read images artifact: {e}"),
                )
            })?;

            let mut all_ocr_text = Vec::new();

            if images.is_empty() {
                // Treat blob content as a single image
                let mime_type = blob
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let entities =
                    ocr::detect_ocr(&self.bridge, &blob.content, &mime_type, &config).await?;
                for entity in &entities {
                    all_ocr_text.push(entity.value.clone());
                    blob.add_artifact("entities", entity).map_err(|e| {
                        Error::new(
                            ErrorKind::Runtime,
                            format!("failed to add entity: {e}"),
                        )
                    })?;
                    count += 1;
                }
            } else {
                for img in &images {
                    let entities =
                        ocr::detect_ocr(&self.bridge, &img.image_data, &img.mime_type, &config)
                            .await?;
                    for entity in &entities {
                        all_ocr_text.push(entity.value.clone());
                        blob.add_artifact("entities", entity).map_err(|e| {
                            Error::new(
                                ErrorKind::Runtime,
                                format!("failed to add entity: {e}"),
                            )
                        })?;
                        count += 1;
                    }
                }
            }

            // Create a Document from concatenated OCR text for downstream processing
            if !all_ocr_text.is_empty() {
                let ocr_doc = Document::new(all_ocr_text.join("\n"))
                    .with_source_format("ocr");
                blob.add_artifact("documents", &ocr_doc).map_err(|e| {
                    Error::new(
                        ErrorKind::Runtime,
                        format!("failed to add OCR document: {e}"),
                    )
                })?;
            }

            if output.send(blob).await.is_err() {
                return Ok(count);
            }
        }

        Ok(count)
    }
}

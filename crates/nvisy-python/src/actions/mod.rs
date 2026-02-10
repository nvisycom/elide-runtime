//! Pipeline actions that perform AI-powered named-entity recognition.
//!
//! Two actions are provided:
//! - [`DetectNerAction`] -- runs NER over text documents.
//! - [`DetectNerImageAction`] -- runs NER over images (OCR + entity detection).

use std::any::Any;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::Document;
use nvisy_core::datatypes::image::ImageData;
use nvisy_core::error::Error;
use nvisy_core::traits::action::Action;
use crate::bridge::PythonBridge;
use crate::ner::{self, NerConfig};

/// Pipeline action that detects named entities in text documents.
///
/// If the incoming [`Blob`] carries `"documents"` artifacts, each document's
/// text is sent through the NER model.  Otherwise the raw blob content is
/// interpreted as UTF-8 text.  Detected entities are stored as `"entities"`
/// artifacts on the blob.
pub struct DetectNerAction;

#[async_trait::async_trait]
impl Action for DetectNerAction {
    fn id(&self) -> &str { "detect-ner" }
    fn requires_client(&self) -> bool { true }
    fn required_provider_id(&self) -> Option<&str> { Some("ai") }

    fn validate_params(&self, _params: &serde_json::Value) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        params: serde_json::Value,
        client: Option<Box<dyn Any + Send>>,
    ) -> Result<u64, Error> {
        let bridge = extract_bridge(client)?;
        let config = parse_ner_config(&params);
        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            let documents: Vec<Document> = blob.get_artifacts("documents")
                .map_err(|e| Error::runtime(format!("Failed to get document artifacts: {}", e), "python/ner", false))?;

            let docs = if documents.is_empty() {
                let text = String::from_utf8(blob.content.to_vec())
                    .map_err(|e| Error::runtime(format!("Blob content is not valid UTF-8: {}", e), "python/ner", false))?;
                vec![Document::new(text)]
            } else {
                documents
            };

            for doc in &docs {
                let entities = ner::detect_ner(&bridge, &doc.content, &config).await?;
                for entity in &entities {
                    blob.add_artifact("entities", entity)
                        .map_err(|e| Error::runtime(format!("Failed to add entity artifact: {}", e), "python/ner", false))?;
                    count += 1;
                }
            }

            if output.send(blob).await.is_err() {
                return Ok(count);
            }
        }

        Ok(count)
    }
}

/// Pipeline action that detects named entities in images.
///
/// If the incoming [`Blob`] carries `"images"` artifacts, each image is
/// processed individually.  Otherwise the raw blob content is treated as a
/// single image whose MIME type is inferred from the blob metadata.
/// Detected entities are stored as `"entities"` artifacts on the blob.
pub struct DetectNerImageAction;

#[async_trait::async_trait]
impl Action for DetectNerImageAction {
    fn id(&self) -> &str { "detect-ner-image" }
    fn requires_client(&self) -> bool { true }
    fn required_provider_id(&self) -> Option<&str> { Some("ai") }

    fn validate_params(&self, _params: &serde_json::Value) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        params: serde_json::Value,
        client: Option<Box<dyn Any + Send>>,
    ) -> Result<u64, Error> {
        let bridge = extract_bridge(client)?;
        let config = parse_ner_config(&params);
        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            let images: Vec<ImageData> = blob.get_artifacts("images")
                .map_err(|e| Error::runtime(format!("Failed to get image artifacts: {}", e), "python/ner-image", false))?;

            if images.is_empty() {
                let mime_type = blob.content_type().unwrap_or("application/octet-stream").to_string();
                let entities = ner::detect_ner_image(
                    &bridge,
                    &blob.content,
                    &mime_type,
                    &config,
                ).await?;
                for entity in &entities {
                    blob.add_artifact("entities", entity)
                        .map_err(|e| Error::runtime(format!("Failed to add entity artifact: {}", e), "python/ner-image", false))?;
                    count += 1;
                }
            } else {
                for img in &images {
                    let entities = ner::detect_ner_image(
                        &bridge,
                        &img.image_data,
                        &img.mime_type,
                        &config,
                    ).await?;
                    for entity in &entities {
                        blob.add_artifact("entities", entity)
                            .map_err(|e| Error::runtime(format!("Failed to add entity artifact: {}", e), "python/ner-image", false))?;
                        count += 1;
                    }
                }
            }

            if output.send(blob).await.is_err() {
                return Ok(count);
            }
        }

        Ok(count)
    }
}

/// Downcast the opaque provider client to a [`PythonBridge`].
fn extract_bridge(client: Option<Box<dyn Any + Send>>) -> Result<PythonBridge, Error> {
    client
        .ok_or_else(|| Error::runtime("AI provider client required", "python", false))?
        .downcast::<PythonBridge>()
        .map(|b| *b)
        .map_err(|_| Error::runtime("Invalid client type for AI actions", "python", false))
}

/// Extract [`NerConfig`] from the action's JSON parameters.
fn parse_ner_config(params: &serde_json::Value) -> NerConfig {
    NerConfig {
        entity_types: params
            .get("entityTypes")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        confidence_threshold: params
            .get("confidenceThreshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5),
        temperature: params
            .get("temperature")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        api_key: params
            .get("apiKey")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        model: params
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("gpt-4")
            .to_string(),
        provider: params
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("openai")
            .to_string(),
    }
}

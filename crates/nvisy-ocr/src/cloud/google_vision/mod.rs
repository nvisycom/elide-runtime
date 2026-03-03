//! [`OcrBackend`] implementation for Google Cloud Vision API.

use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;

use nvisy_core::Error;
use nvisy_core::math::{BoundingBox, Polygon, Vertex};
use nvisy_ontology::location::TextLevel;

use crate::backend::{ImageInput, OcrBackend, OcrConfig, OcrRegion};

/// Remote OCR backend using Google Cloud Vision API.
///
/// Sends images as base64-encoded JSON to the `images:annotate` endpoint
/// and parses word-level results from the `fullTextAnnotation` response.
pub struct GoogleVisionBackend {
    client: ClientWithMiddleware,
    api_key: String,
}

impl GoogleVisionBackend {
    /// Create a new backend with the given HTTP client and API key.
    pub fn new(client: ClientWithMiddleware, api_key: impl Into<String>) -> Self {
        Self {
            client,
            api_key: api_key.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct AnnotateResponse {
    responses: Vec<AnnotateResult>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnnotateResult {
    full_text_annotation: Option<FullTextAnnotation>,
}

#[derive(Debug, Deserialize)]
struct FullTextAnnotation {
    pages: Vec<GvPage>,
}

#[derive(Debug, Deserialize)]
struct GvPage {
    blocks: Vec<GvBlock>,
}

#[derive(Debug, Deserialize)]
struct GvBlock {
    paragraphs: Vec<GvParagraph>,
}

#[derive(Debug, Deserialize)]
struct GvParagraph {
    words: Vec<GvWord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GvWord {
    symbols: Vec<GvSymbol>,
    #[serde(default)]
    confidence: f64,
    bounding_box: Option<GvBoundingPoly>,
}

#[derive(Debug, Deserialize)]
struct GvSymbol {
    text: String,
}

#[derive(Debug, Deserialize)]
struct GvBoundingPoly {
    vertices: Vec<GvVertex>,
}

#[derive(Debug, Deserialize)]
struct GvVertex {
    x: Option<i32>,
    y: Option<i32>,
}

#[async_trait::async_trait]
impl OcrBackend for GoogleVisionBackend {
    async fn run(
        &self,
        image: &ImageInput,
        config: &OcrConfig,
    ) -> Result<Vec<OcrRegion>, Error> {
        let encoded = image.to_base64();

        let body = serde_json::json!({
            "requests": [{
                "image": { "content": encoded },
                "features": [{ "type": "DOCUMENT_TEXT_DETECTION" }]
            }]
        });

        let url = format!(
            "https://vision.googleapis.com/v1/images:annotate?key={}",
            self.api_key
        );

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                Error::runtime(
                    format!("Google Vision request failed: {e}"),
                    "google_vision_ocr",
                    false,
                )
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::runtime(
                format!("Google Vision returned {status}: {body}"),
                "google_vision_ocr",
                false,
            ));
        }

        let parsed: AnnotateResponse = resp.json().await.map_err(|e| {
            Error::runtime(
                format!("failed to parse Google Vision response: {e}"),
                "google_vision_ocr",
                false,
            )
        })?;

        let threshold = config.confidence_threshold;
        let mut regions = Vec::new();

        for result in &parsed.responses {
            let annotation = match &result.full_text_annotation {
                Some(a) => a,
                None => continue,
            };

            for page in &annotation.pages {
                for block in &page.blocks {
                    for paragraph in &block.paragraphs {
                        for word in &paragraph.words {
                            if word.confidence < threshold {
                                continue;
                            }

                            let text: String =
                                word.symbols.iter().map(|s| s.text.as_str()).collect();

                            let polygon = word.bounding_box.as_ref().map(|bp| {
                                Polygon {
                                    vertices: bp
                                        .vertices
                                        .iter()
                                        .map(|v| {
                                            Vertex::new(
                                                f64::from(v.x.unwrap_or(0)),
                                                f64::from(v.y.unwrap_or(0)),
                                            )
                                        })
                                        .collect(),
                                }
                            });

                            let bbox = polygon
                                .as_ref()
                                .map(|p| p.bounding_box())
                                .unwrap_or(BoundingBox {
                                    x: 0.0,
                                    y: 0.0,
                                    width: 0.0,
                                    height: 0.0,
                                });

                            regions.push(OcrRegion {
                                text,
                                confidence: word.confidence,
                                bbox,
                                polygon,
                                level: Some(TextLevel::Word),
                            });
                        }
                    }
                }
            }
        }

        Ok(regions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_response() {
        let json = serde_json::json!({
            "responses": [{
                "fullTextAnnotation": {
                    "pages": [{
                        "blocks": [{
                            "paragraphs": [{
                                "words": [
                                    {
                                        "symbols": [
                                            { "text": "H" },
                                            { "text": "i" }
                                        ],
                                        "confidence": 0.99,
                                        "boundingBox": {
                                            "vertices": [
                                                { "x": 10, "y": 20 },
                                                { "x": 50, "y": 20 },
                                                { "x": 50, "y": 40 },
                                                { "x": 10, "y": 40 }
                                            ]
                                        }
                                    }
                                ]
                            }]
                        }]
                    }]
                }
            }]
        });

        let resp: AnnotateResponse = serde_json::from_value(json).unwrap();
        let annotation = resp.responses[0].full_text_annotation.as_ref().unwrap();
        let word = &annotation.pages[0].blocks[0].paragraphs[0].words[0];

        let text: String = word.symbols.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(text, "Hi");
        assert!((word.confidence - 0.99).abs() < 0.001);

        let bp = word.bounding_box.as_ref().unwrap();
        assert_eq!(bp.vertices.len(), 4);
        assert_eq!(bp.vertices[0].x, Some(10));
        assert_eq!(bp.vertices[0].y, Some(20));
    }

    #[test]
    fn handles_missing_vertex_coords() {
        let json = serde_json::json!({
            "responses": [{
                "fullTextAnnotation": {
                    "pages": [{
                        "blocks": [{
                            "paragraphs": [{
                                "words": [{
                                    "symbols": [{ "text": "A" }],
                                    "confidence": 0.9,
                                    "boundingBox": {
                                        "vertices": [
                                            {},
                                            { "x": 100 },
                                            { "y": 200 },
                                            { "x": 100, "y": 200 }
                                        ]
                                    }
                                }]
                            }]
                        }]
                    }]
                }
            }]
        });

        let resp: AnnotateResponse = serde_json::from_value(json).unwrap();
        let word = &resp.responses[0]
            .full_text_annotation
            .as_ref()
            .unwrap()
            .pages[0]
            .blocks[0]
            .paragraphs[0]
            .words[0];
        let bp = word.bounding_box.as_ref().unwrap();

        assert_eq!(bp.vertices[0].x, None);
        assert_eq!(bp.vertices[0].y, None);
        assert_eq!(bp.vertices[1].x, Some(100));
        assert_eq!(bp.vertices[1].y, None);
    }

    #[test]
    fn filters_below_threshold() {
        let json = serde_json::json!({
            "responses": [{
                "fullTextAnnotation": {
                    "pages": [{
                        "blocks": [{
                            "paragraphs": [{
                                "words": [
                                    {
                                        "symbols": [{ "text": "low" }],
                                        "confidence": 0.2,
                                        "boundingBox": {
                                            "vertices": [
                                                { "x": 0, "y": 0 },
                                                { "x": 10, "y": 0 },
                                                { "x": 10, "y": 10 },
                                                { "x": 0, "y": 10 }
                                            ]
                                        }
                                    },
                                    {
                                        "symbols": [{ "text": "high" }],
                                        "confidence": 0.95,
                                        "boundingBox": {
                                            "vertices": [
                                                { "x": 20, "y": 0 },
                                                { "x": 40, "y": 0 },
                                                { "x": 40, "y": 10 },
                                                { "x": 20, "y": 10 }
                                            ]
                                        }
                                    }
                                ]
                            }]
                        }]
                    }]
                }
            }]
        });

        let resp: AnnotateResponse = serde_json::from_value(json).unwrap();
        let threshold = 0.5;

        let words: Vec<_> = resp.responses[0]
            .full_text_annotation
            .as_ref()
            .unwrap()
            .pages[0]
            .blocks[0]
            .paragraphs[0]
            .words
            .iter()
            .filter(|w| w.confidence >= threshold)
            .collect();

        assert_eq!(words.len(), 1);
        let text: String = words[0].symbols.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(text, "high");
    }
}

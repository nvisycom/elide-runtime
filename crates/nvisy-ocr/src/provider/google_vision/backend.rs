//! [`Backend`] implementation for Google Cloud Vision API.
//!
//! [`Backend`]: crate::Backend

use std::fmt;

use serde::Deserialize;

use nvisy_core::Error;
use nvisy_rig::backend::{HttpConfig, build_http_client};
use nvisy_core::math::{Polygon, Vertex};
use nvisy_ontology::location::TextLevel;
use reqwest_middleware::ClientWithMiddleware;

use crate::backend::{ImageInput, ImageOutput, Backend, ImageRegion, RunParams, check_response};

use super::GoogleVisionParams;

/// [`Backend`] implementation for Google Cloud Vision API.
///
/// Sends images as base64-encoded JSON to the `images:annotate` endpoint
/// and parses word-level results from the `fullTextAnnotation` response.
///
/// [`Backend`]: crate::Backend
pub struct GoogleVisionBackend {
    client: ClientWithMiddleware,
    api_key: String,
}

impl fmt::Debug for GoogleVisionBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GoogleVisionBackend")
            .field("api_key", &"***")
            .finish()
    }
}

impl GoogleVisionBackend {
    /// Create a new backend with default HTTP configuration.
    pub fn new(params: GoogleVisionParams) -> Self {
        Self::with_client(build_http_client(&HttpConfig::default()), params)
    }

    /// Create a new backend with a pre-configured HTTP client.
    pub fn with_client(client: ClientWithMiddleware, params: GoogleVisionParams) -> Self {
        Self {
            client,
            api_key: params.api_key,
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
impl Backend for GoogleVisionBackend {
    async fn run(
        &self,
        image: &ImageInput,
        params: &RunParams,
    ) -> Result<ImageOutput, Error> {
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
            .map_err(|e| Error::connection(e.to_string(), "google_vision_ocr", true))?;

        let resp = check_response(resp, "Google Vision").await?;

        let parsed: AnnotateResponse = resp
            .json()
            .await
            .map_err(|e| Error::runtime(format!("Google Vision JSON parse error: {e}"), "google_vision_ocr", false))?;

        let threshold = params.confidence_threshold;
        let mut output = ImageOutput::new(image.source.derive());

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
                                .unwrap_or_default();

                            output.insert(ImageRegion {
                                text,
                                confidence: Some(word.confidence),
                                bbox,
                                polygon,
                                level: Some(TextLevel::Word),
                            });
                        }
                    }
                }
            }
        }

        Ok(output)
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
    fn concatenates_symbols_into_word() {
        let json = serde_json::json!({
            "responses": [{
                "fullTextAnnotation": {
                    "pages": [{
                        "blocks": [{
                            "paragraphs": [{
                                "words": [{
                                    "symbols": [
                                        { "text": "H" },
                                        { "text": "e" },
                                        { "text": "l" },
                                        { "text": "l" },
                                        { "text": "o" }
                                    ],
                                    "confidence": 0.99,
                                    "boundingBox": {
                                        "vertices": [
                                            { "x": 0, "y": 0 },
                                            { "x": 50, "y": 0 },
                                            { "x": 50, "y": 20 },
                                            { "x": 0, "y": 20 }
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

        let text: String = word.symbols.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(text, "Hello");
    }
}

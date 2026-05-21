//! [`Backend`] implementation for Google Cloud Vision API.
//!
//! [`Backend`]: crate::Backend

use std::fmt;

use nvisy_core::{Error, Result};
use nvisy_http::{HttpClient, HttpConfig, RequestBuilderExt};
use nvisy_ontology::artifacts::{Block, BlockKind, Line, Page, Word};
use nvisy_ontology::primitive::{BoundingBox, Polygon, Vertex};
use serde::Deserialize;

use super::GoogleVisionParams;
use crate::ocr::backend::{Backend, ImageInput, ImageOutput, RunParams};

/// [`Backend`] implementation for Google Cloud Vision API.
///
/// Sends images as base64-encoded JSON to the `images:annotate` endpoint
/// and parses the `fullTextAnnotation` response into a hierarchical
/// page/block/line/word tree.
///
/// [`Backend`]: crate::Backend
pub struct GoogleVisionBackend {
    client: HttpClient,
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
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn new(params: GoogleVisionParams) -> Result<Self> {
        Ok(Self::with_client(
            HttpClient::new(&HttpConfig::default())?,
            params,
        ))
    }

    /// Create a new backend with a pre-configured HTTP client.
    pub fn with_client(client: HttpClient, params: GoogleVisionParams) -> Self {
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

fn gv_polygon(bp: &GvBoundingPoly) -> Polygon {
    Polygon {
        vertices: bp
            .vertices
            .iter()
            .map(|v| Vertex::new(f64::from(v.x.unwrap_or(0)), f64::from(v.y.unwrap_or(0))))
            .collect(),
    }
}

fn gv_bbox_polygon(bp: Option<&GvBoundingPoly>) -> (BoundingBox, Option<Polygon>) {
    match bp {
        Some(bp) => {
            let polygon = gv_polygon(bp);
            let bbox = polygon.bounding_box();
            (bbox, Some(polygon))
        }
        None => (BoundingBox::default(), None),
    }
}

#[async_trait::async_trait]
impl Backend for GoogleVisionBackend {
    async fn run(&self, image: &ImageInput, params: &RunParams) -> Result<ImageOutput, Error> {
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

        let parsed: AnnotateResponse = self
            .client
            .post(&url)
            .json(&body)
            .send_and_parse("google_vision_ocr")
            .await?;

        let threshold = params.confidence_threshold;
        let mut output = ImageOutput::new(image.source.derive());

        for (result_idx, result) in parsed.responses.iter().enumerate() {
            let annotation = match &result.full_text_annotation {
                Some(a) => a,
                None => continue,
            };

            for (page_idx, gv_page) in annotation.pages.iter().enumerate() {
                let mut blocks = Vec::new();

                for gv_block in &gv_page.blocks {
                    let mut lines = Vec::new();

                    // Each GV paragraph maps to a Line.
                    for paragraph in &gv_block.paragraphs {
                        let mut words = Vec::new();

                        for gv_word in &paragraph.words {
                            if gv_word.confidence < threshold {
                                continue;
                            }

                            let text: String =
                                gv_word.symbols.iter().map(|s| s.text.as_str()).collect();

                            let (bbox, polygon) = gv_bbox_polygon(gv_word.bounding_box.as_ref());

                            words.push(Word {
                                text,
                                confidence: Some(gv_word.confidence),
                                bbox,
                                polygon,
                            });
                        }

                        if words.is_empty() {
                            continue;
                        }

                        let line_text = words
                            .iter()
                            .map(|w| w.text.as_str())
                            .collect::<Vec<_>>()
                            .join(" ");
                        let line_bbox = BoundingBox::enclosing(words.iter().map(|w| &w.bbox));

                        lines.push(Line {
                            text: line_text,
                            confidence: None,
                            bbox: line_bbox,
                            polygon: None,
                            words,
                        });
                    }

                    if lines.is_empty() {
                        continue;
                    }

                    let block_text = lines
                        .iter()
                        .map(|l| l.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    let block_bbox = BoundingBox::enclosing(lines.iter().map(|l| &l.bbox));

                    blocks.push(Block {
                        text: block_text,
                        confidence: None,
                        bbox: block_bbox,
                        polygon: None,
                        kind: BlockKind::Text,
                        lines,
                    });
                }

                output.pages.push(Page {
                    page_number: (result_idx + page_idx + 1) as u32,
                    width: None,
                    height: None,
                    blocks,
                });
            }
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

//! [`Backend`] implementation for Surya OCR.
//!
//! [`Backend`]: crate::Backend

use nvisy_core::{Error, Result};
use nvisy_ontology::artifacts::{Block, BlockKind, Line, Page, Word};
use nvisy_ontology::math::{BoundingBox, Polygon, Vertex};
use reqwest_middleware::reqwest::multipart::Form;
use serde::Deserialize;

use super::SuryaParams;
use crate::http::{HttpClient, HttpConfig};
use crate::ocr::backend::{Backend, ImageInput, ImageOutput, RunParams, image_part};

/// [`Backend`] implementation for Surya OCR.
///
/// Sends images as multipart form data to `{base_url}/ocr` and parses
/// the response into a hierarchical page/block/line/word tree.
/// Surya's `TextLine` maps directly to [`Line`].
///
/// [`Backend`]: crate::Backend
#[derive(Debug)]
pub struct SuryaBackend {
    client: HttpClient,
    base_url: String,
}

impl SuryaBackend {
    /// Create a new backend with default HTTP configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn new(params: SuryaParams) -> Result<Self> {
        Ok(Self::with_client(
            HttpClient::new(&HttpConfig::default())?,
            params,
        ))
    }

    /// Create a new backend with a pre-configured HTTP client.
    pub fn with_client(client: HttpClient, params: SuryaParams) -> Self {
        Self {
            client,
            base_url: params.base_url,
        }
    }
}

#[derive(Debug, Deserialize)]
struct SuryaResponse {
    pages: Vec<SuryaPage>,
}

#[derive(Debug, Deserialize)]
struct SuryaPage {
    /// Upstream page number (0-based).
    page: u32,
    /// Document image bounds `[x_min, y_min, x_max, y_max]`.
    image_bbox: [f64; 4],
    text_lines: Vec<SuryaTextLine>,
}

#[derive(Debug, Deserialize)]
struct SuryaTextLine {
    words: Vec<SuryaWord>,
}

#[derive(Debug, Deserialize)]
struct SuryaWord {
    text: String,
    #[serde(default)]
    confidence: f64,
    /// `[x_min, y_min, x_max, y_max]` in pixel coordinates.
    bbox: [f64; 4],
    /// 4-point polygon `[[x1,y1], [x2,y2], [x3,y3], [x4,y4]]`.
    polygon: [[f64; 2]; 4],
}

#[async_trait::async_trait]
impl Backend for SuryaBackend {
    async fn run(&self, image: &ImageInput, params: &RunParams) -> Result<ImageOutput, Error> {
        let file_part = image_part(image)?;

        let form = Form::new().part("file", file_part);

        let url = format!("{}/ocr", self.base_url.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| Error::connection(e.to_string(), "surya_ocr", true))?;

        let resp = HttpClient::check_response(resp, "Surya").await?;

        let parsed: SuryaResponse = resp.json().await.map_err(|e| {
            Error::runtime(format!("Surya JSON parse error: {e}"), "surya_ocr", false)
        })?;

        let threshold = params.confidence_threshold;
        let mut output = ImageOutput::new(image.source.derive());

        for surya_page in &parsed.pages {
            let mut lines = Vec::new();

            for text_line in &surya_page.text_lines {
                let mut words = Vec::new();

                for word in &text_line.words {
                    if word.confidence < threshold {
                        continue;
                    }

                    let [x_min, y_min, x_max, y_max] = word.bbox;

                    let bbox = BoundingBox {
                        x: x_min,
                        y: y_min,
                        width: x_max - x_min,
                        height: y_max - y_min,
                    };

                    let polygon = Polygon {
                        vertices: word
                            .polygon
                            .iter()
                            .map(|[x, y]| Vertex::new(*x, *y))
                            .collect(),
                    };

                    words.push(Word {
                        text: word.text.clone(),
                        confidence: Some(word.confidence),
                        bbox,
                        polygon: Some(polygon),
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

            let block_text = lines
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let block_bbox = BoundingBox::enclosing(lines.iter().map(|l| &l.bbox));

            let [_x_min, _y_min, x_max, y_max] = surya_page.image_bbox;

            output.pages.push(Page {
                page_number: surya_page.page + 1,
                width: Some(x_max),
                height: Some(y_max),
                blocks: vec![Block {
                    text: block_text,
                    confidence: None,
                    bbox: block_bbox,
                    polygon: None,
                    kind: BlockKind::Text,
                    lines,
                }],
            });
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
            "pages": [{
                "page": 0,
                "image_bbox": [0.0, 0.0, 800.0, 600.0],
                "text_lines": [{
                    "words": [
                        {
                            "text": "hello",
                            "confidence": 0.95,
                            "bbox": [10.0, 20.0, 60.0, 40.0],
                            "polygon": [[10, 20], [60, 20], [60, 40], [10, 40]]
                        },
                        {
                            "text": "world",
                            "confidence": 0.93,
                            "bbox": [65.0, 20.0, 110.0, 40.0],
                            "polygon": [[65, 20], [110, 20], [110, 40], [65, 40]]
                        }
                    ]
                }]
            }]
        });

        let resp: SuryaResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.pages.len(), 1);
        assert_eq!(resp.pages[0].text_lines.len(), 1);
        assert_eq!(resp.pages[0].text_lines[0].words.len(), 2);

        let word = &resp.pages[0].text_lines[0].words[0];
        assert_eq!(word.text, "hello");
        assert!((word.confidence - 0.95).abs() < 0.001);

        let [x_min, y_min, x_max, y_max] = word.bbox;
        let bbox = BoundingBox {
            x: x_min,
            y: y_min,
            width: x_max - x_min,
            height: y_max - y_min,
        };
        assert!((bbox.x - 10.0).abs() < 0.01);
        assert!((bbox.y - 20.0).abs() < 0.01);
        assert!((bbox.width - 50.0).abs() < 0.01);
        assert!((bbox.height - 20.0).abs() < 0.01);

        let polygon = Polygon {
            vertices: word
                .polygon
                .iter()
                .map(|[x, y]| Vertex::new(*x, *y))
                .collect(),
        };
        assert_eq!(polygon.vertices.len(), 4);
        assert!((polygon.vertices[0].x - 10.0).abs() < 0.01);
        assert!((polygon.vertices[0].y - 20.0).abs() < 0.01);
    }
}

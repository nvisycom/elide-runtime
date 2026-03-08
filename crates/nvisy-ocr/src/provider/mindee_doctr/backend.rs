//! [`Backend`] implementation for DocTR.
//!
//! [`Backend`]: crate::Backend

use nvisy_core::Error;
use nvisy_core::math::{BoundingBox, Polygon, Vertex};
use nvisy_http::HttpClient;
use reqwest_middleware::reqwest::multipart::Form;
use serde::Deserialize;

use super::DoctrParams;
use crate::backend::{
    Backend, Block, BlockKind, ImageInput, ImageOutput, Line, Page, RunParams, Word,
    check_response, image_part,
};

/// [`Backend`] implementation for DocTR.
///
/// Sends images as multipart form data to `{base_url}/ocr` and parses
/// word-level results into a hierarchical tree. DocTR returns normalised
/// 0..1 coordinates that are denormalised using the `dimensions` field.
///
/// [`Backend`]: crate::Backend
#[derive(Debug)]
pub struct DoctrBackend {
    client: HttpClient,
    base_url: String,
}

impl DoctrBackend {
    /// Create a new backend with default HTTP configuration.
    pub fn new(params: DoctrParams) -> Self {
        Self::with_client(HttpClient::default(), params)
    }

    /// Create a new backend with a pre-configured HTTP client.
    pub fn with_client(client: HttpClient, params: DoctrParams) -> Self {
        Self {
            client,
            base_url: params.base_url,
        }
    }
}

#[derive(Debug, Deserialize)]
struct DoctrResponse {
    pages: Vec<DoctrPage>,
}

#[derive(Debug, Deserialize)]
struct DoctrPage {
    /// `[height, width]` in pixels.
    dimensions: [f64; 2],
    words: Vec<DoctrWord>,
}

#[derive(Debug, Deserialize)]
struct DoctrWord {
    value: String,
    #[serde(default)]
    confidence: f64,
    /// `[[x_min, y_min], [x_max, y_max]]` in normalised 0–1 coords.
    geometry: [[f64; 2]; 2],
}

#[async_trait::async_trait]
impl Backend for DoctrBackend {
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
            .map_err(|e| Error::connection(e.to_string(), "doctr_ocr", true))?;

        let resp = check_response(resp, "DocTR").await?;

        let parsed: DoctrResponse = resp.json().await.map_err(|e| {
            Error::runtime(format!("DocTR JSON parse error: {e}"), "doctr_ocr", false)
        })?;

        let threshold = params.confidence_threshold;
        let mut output = ImageOutput::new(image.source.derive());

        for (page_idx, page) in parsed.pages.iter().enumerate() {
            let [height, width] = page.dimensions;
            let mut words = Vec::new();

            for word in &page.words {
                if word.confidence < threshold {
                    continue;
                }

                let [[x_min_n, y_min_n], [x_max_n, y_max_n]] = word.geometry;

                let x_min = x_min_n * width;
                let y_min = y_min_n * height;
                let x_max = x_max_n * width;
                let y_max = y_max_n * height;

                let polygon = Polygon {
                    vertices: vec![
                        Vertex::new(x_min, y_min), // TL
                        Vertex::new(x_max, y_min), // TR
                        Vertex::new(x_max, y_max), // BR
                        Vertex::new(x_min, y_max), // BL
                    ],
                };
                let bbox = polygon.bounding_box();

                words.push(Word {
                    text: word.value.clone(),
                    confidence: Some(word.confidence),
                    bbox,
                    polygon: Some(polygon),
                });
            }

            let line_text = words
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let line_bbox =
                BoundingBox::enclosing(words.iter().map(|w| &w.bbox));

            let line = Line {
                text: line_text.clone(),
                confidence: None,
                bbox: line_bbox,
                polygon: None,
                words,
            };

            let block = Block {
                text: line_text,
                confidence: None,
                bbox: line_bbox,
                polygon: None,
                kind: BlockKind::Text,
                lines: vec![line],
            };

            output.pages.push(Page {
                page_number: (page_idx + 1) as u32,
                width: Some(width),
                height: Some(height),
                blocks: vec![block],
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
                "dimensions": [1000.0, 2000.0],
                "words": [
                    {
                        "value": "hello",
                        "confidence": 0.97,
                        "geometry": [[0.05, 0.10], [0.15, 0.14]]
                    },
                    {
                        "value": "world",
                        "confidence": 0.95,
                        "geometry": [[0.20, 0.10], [0.30, 0.14]]
                    }
                ]
            }]
        });

        let resp: DoctrResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.pages.len(), 1);
        assert_eq!(resp.pages[0].words.len(), 2);

        let page = &resp.pages[0];
        let [height, width] = page.dimensions;
        let word = &page.words[0];

        // Denormalise: x_min = 0.05 * 2000 = 100, y_min = 0.10 * 1000 = 100
        let x_min = word.geometry[0][0] * width;
        let y_min = word.geometry[0][1] * height;
        let x_max = word.geometry[1][0] * width;
        let y_max = word.geometry[1][1] * height;

        assert!((x_min - 100.0).abs() < 0.01);
        assert!((y_min - 100.0).abs() < 0.01);
        assert!((x_max - 300.0).abs() < 0.01);
        assert!((y_max - 140.0).abs() < 0.01);

        let polygon = Polygon {
            vertices: vec![
                Vertex::new(x_min, y_min),
                Vertex::new(x_max, y_min),
                Vertex::new(x_max, y_max),
                Vertex::new(x_min, y_max),
            ],
        };
        let bbox = polygon.bounding_box();
        assert!((bbox.x - 100.0).abs() < 0.01);
        assert!((bbox.y - 100.0).abs() < 0.01);
        assert!((bbox.width - 200.0).abs() < 0.01);
        assert!((bbox.height - 40.0).abs() < 0.01);
    }
}

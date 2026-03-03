//! [`OcrBackend`] implementation for DocTR.

use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;

use nvisy_core::Error;
use nvisy_core::math::{Polygon, Vertex};
use nvisy_ontology::location::TextLevel;

use crate::backend::{ImageInput, OcrBackend, OcrConfig, OcrRegion};

/// Remote OCR backend using a DocTR server.
///
/// Sends images as multipart form data to `{base_url}/ocr` and parses
/// word-level results into [`OcrRegion`]. DocTR returns normalised 0–1
/// coordinates which are denormalised using the `dimensions` field from
/// the response.
pub struct DoctrBackend {
    client: ClientWithMiddleware,
    base_url: String,
}

impl DoctrBackend {
    /// Create a new backend with the given HTTP client and server URL.
    pub fn new(client: ClientWithMiddleware, base_url: impl Into<String>) -> Self {
        Self {
            client,
            base_url: base_url.into(),
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
impl OcrBackend for DoctrBackend {
    async fn run(
        &self,
        image: &ImageInput,
        config: &OcrConfig,
    ) -> Result<Vec<OcrRegion>, Error> {
        let file_part = reqwest_middleware::reqwest::multipart::Part::bytes(image.data.to_vec())
            .file_name("image")
            .mime_str(image.mime_type())
            .map_err(|e| Error::runtime(format!("invalid mime type: {e}"), "doctr_ocr", false))?;

        let form = reqwest_middleware::reqwest::multipart::Form::new().part("file", file_part);

        let url = format!("{}/ocr", self.base_url.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                Error::runtime(format!("DocTR request failed: {e}"), "doctr_ocr", false)
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::runtime(
                format!("DocTR returned {status}: {body}"),
                "doctr_ocr",
                false,
            ));
        }

        let parsed: DoctrResponse = resp.json().await.map_err(|e| {
            Error::runtime(
                format!("failed to parse DocTR response: {e}"),
                "doctr_ocr",
                false,
            )
        })?;

        let threshold = config.confidence_threshold;
        let mut regions = Vec::new();

        for page in &parsed.pages {
            let [height, width] = page.dimensions;

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

                regions.push(OcrRegion {
                    text: word.value.clone(),
                    confidence: word.confidence,
                    bbox,
                    polygon: Some(polygon),
                    level: Some(TextLevel::Word),
                });
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

    #[test]
    fn filters_below_threshold() {
        let json = serde_json::json!({
            "pages": [{
                "dimensions": [100.0, 100.0],
                "words": [
                    {
                        "value": "low",
                        "confidence": 0.2,
                        "geometry": [[0.0, 0.0], [0.5, 0.5]]
                    },
                    {
                        "value": "high",
                        "confidence": 0.9,
                        "geometry": [[0.5, 0.0], [1.0, 0.5]]
                    }
                ]
            }]
        });

        let resp: DoctrResponse = serde_json::from_value(json).unwrap();
        let threshold = 0.5;

        let regions: Vec<OcrRegion> = resp
            .pages
            .iter()
            .flat_map(|page| {
                let [height, width] = page.dimensions;
                page.words.iter().filter_map(move |word| {
                    if word.confidence < threshold {
                        return None;
                    }
                    let x_min = word.geometry[0][0] * width;
                    let y_min = word.geometry[0][1] * height;
                    let x_max = word.geometry[1][0] * width;
                    let y_max = word.geometry[1][1] * height;
                    let polygon = Polygon {
                        vertices: vec![
                            Vertex::new(x_min, y_min),
                            Vertex::new(x_max, y_min),
                            Vertex::new(x_max, y_max),
                            Vertex::new(x_min, y_max),
                        ],
                    };
                    let bbox = polygon.bounding_box();
                    Some(OcrRegion {
                        text: word.value.clone(),
                        confidence: word.confidence,
                        bbox,
                        polygon: Some(polygon),
                        level: Some(TextLevel::Word),
                    })
                })
            })
            .collect();

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].text, "high");
    }

    #[test]
    fn multi_page_response() {
        let json = serde_json::json!({
            "pages": [
                {
                    "dimensions": [100.0, 200.0],
                    "words": [{
                        "value": "page1",
                        "confidence": 0.99,
                        "geometry": [[0.0, 0.0], [0.5, 0.5]]
                    }]
                },
                {
                    "dimensions": [300.0, 400.0],
                    "words": [{
                        "value": "page2",
                        "confidence": 0.98,
                        "geometry": [[0.1, 0.1], [0.9, 0.9]]
                    }]
                }
            ]
        });

        let resp: DoctrResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.pages.len(), 2);
        assert_eq!(resp.pages[0].words[0].value, "page1");
        assert_eq!(resp.pages[1].words[0].value, "page2");
    }
}

//! [`Backend`] implementation for Surya OCR.
//!
//! [`Backend`]: crate::Backend

use serde::Deserialize;

use nvisy_core::Error;
use nvisy_core::math::{BoundingBox, Polygon, Vertex};
use nvisy_ontology::location::TextLevel;
use nvisy_rig::backend::{HttpConfig, build_http_client};
use reqwest_middleware::ClientWithMiddleware;
use reqwest_middleware::reqwest::multipart::Form;

use crate::backend::{Backend, ImageInput, ImageOutput, ImageRegion, RunParams, image_part};

use super::SuryaParams;

/// [`Backend`] implementation for Surya OCR.
///
/// Sends images as multipart form data to `{base_url}/ocr` and parses
/// word-level results into [`ImageRegion`]. Surya returns both a 4-point
/// polygon and an axis-aligned bounding box in pixel coordinates.
///
/// [`Backend`]: crate::Backend
/// [`ImageRegion`]: crate::ImageRegion
pub struct SuryaBackend {
    client: ClientWithMiddleware,
    base_url: String,
}

impl SuryaBackend {
    /// Create a new backend with default HTTP configuration.
    pub fn new(params: SuryaParams) -> Self {
        Self::with_client(build_http_client(&HttpConfig::default()), params)
    }

    /// Create a new backend with a pre-configured HTTP client.
    pub fn with_client(client: ClientWithMiddleware, params: SuryaParams) -> Self {
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
    async fn run(
        &self,
        image: &ImageInput,
        params: &RunParams,
    ) -> Result<ImageOutput, Error> {
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

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::connection(
                format!("Surya returned {status}: {body}"),
                "surya_ocr",
                status.is_server_error(),
            ));
        }

        let parsed: SuryaResponse = resp
            .json()
            .await
            .map_err(|e| Error::runtime(format!("Surya JSON parse error: {e}"), "surya_ocr", false))?;

        let threshold = params.confidence_threshold;
        let mut output = ImageOutput::new(image.source.derive());

        for page in &parsed.pages {
            for line in &page.text_lines {
                for word in &line.words {
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

                    output.insert(ImageRegion {
                        text: word.text.clone(),
                        confidence: Some(word.confidence),
                        bbox,
                        polygon: Some(polygon),
                        level: Some(TextLevel::Word),
                    });
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
            "pages": [{
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

    #[test]
    fn filters_below_threshold() {
        let json = serde_json::json!({
            "pages": [{
                "text_lines": [{
                    "words": [
                        {
                            "text": "low",
                            "confidence": 0.3,
                            "bbox": [0, 0, 25, 20],
                            "polygon": [[0, 0], [25, 0], [25, 20], [0, 20]]
                        },
                        {
                            "text": "high",
                            "confidence": 0.9,
                            "bbox": [30, 0, 50, 20],
                            "polygon": [[30, 0], [50, 0], [50, 20], [30, 20]]
                        }
                    ]
                }]
            }]
        });

        let resp: SuryaResponse = serde_json::from_value(json).unwrap();
        let threshold = 0.5;

        let regions: Vec<ImageRegion> = resp
            .pages
            .iter()
            .flat_map(|p| &p.text_lines)
            .flat_map(|l| &l.words)
            .filter(|w| w.confidence >= threshold)
            .map(|w| {
                let [x_min, y_min, x_max, y_max] = w.bbox;
                let bbox = BoundingBox {
                    x: x_min,
                    y: y_min,
                    width: x_max - x_min,
                    height: y_max - y_min,
                };
                let polygon = Polygon {
                    vertices: w.polygon.iter().map(|[x, y]| Vertex::new(*x, *y)).collect(),
                };
                ImageRegion {
                    text: w.text.clone(),
                    confidence: Some(w.confidence),
                    bbox,
                    polygon: Some(polygon),
                    level: Some(TextLevel::Word),
                }
            })
            .collect();

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].text, "high");
    }

    #[test]
    fn multi_line_multi_page() {
        let json = serde_json::json!({
            "pages": [
                {
                    "text_lines": [
                        {
                            "words": [{
                                "text": "line1",
                                "confidence": 0.99,
                                "bbox": [0, 0, 50, 20],
                                "polygon": [[0, 0], [50, 0], [50, 20], [0, 20]]
                            }]
                        },
                        {
                            "words": [{
                                "text": "line2",
                                "confidence": 0.98,
                                "bbox": [0, 30, 50, 50],
                                "polygon": [[0, 30], [50, 30], [50, 50], [0, 50]]
                            }]
                        }
                    ]
                },
                {
                    "text_lines": [{
                        "words": [{
                            "text": "page2",
                            "confidence": 0.97,
                            "bbox": [10, 10, 60, 30],
                            "polygon": [[10, 10], [60, 10], [60, 30], [10, 30]]
                        }]
                    }]
                }
            ]
        });

        let resp: SuryaResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.pages.len(), 2);
        assert_eq!(resp.pages[0].text_lines.len(), 2);
        assert_eq!(resp.pages[1].text_lines.len(), 1);
    }
}

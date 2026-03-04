//! [`Backend`] implementation for PaddleX PP-OCRv5.
//!
//! [`Backend`]: crate::Backend

use serde::Deserialize;

use nvisy_core::Error;
use nvisy_core::math::{Polygon, Vertex};
use crate::backend::TextLevel;
use nvisy_rig::backend::{HttpConfig, build_http_client};
use reqwest_middleware::ClientWithMiddleware;
use reqwest_middleware::reqwest::multipart::Form;

use crate::backend::{Backend, ImageInput, ImageOutput, ImageRegion, RunParams, check_response, image_part};

use super::PaddleXParams;

/// [`Backend`] implementation for PaddleX PP-OCRv5.
///
/// Sends images as multipart form data to `{base_url}/ocr` with
/// `returnWordBox=true` and parses word-level results into [`ImageRegion`].
///
/// [`Backend`]: crate::Backend
/// [`ImageRegion`]: crate::ImageRegion
#[derive(Debug)]
pub struct PaddleXBackend {
    client: ClientWithMiddleware,
    base_url: String,
}

impl PaddleXBackend {
    /// Create a new backend with default HTTP configuration.
    pub fn new(params: PaddleXParams) -> Self {
        Self::with_client(build_http_client(&HttpConfig::default()), params)
    }

    /// Create a new backend with a pre-configured HTTP client.
    pub fn with_client(client: ClientWithMiddleware, params: PaddleXParams) -> Self {
        Self {
            client,
            base_url: params.base_url,
        }
    }
}

#[derive(Debug, Deserialize)]
struct PaddleXResponse {
    result: PaddleXResult,
}

#[derive(Debug, Deserialize)]
struct PaddleXResult {
    #[serde(rename = "ocrResults")]
    ocr_results: Vec<PaddleXOcrResult>,
}

#[derive(Debug, Deserialize)]
struct PaddleXOcrResult {
    #[serde(rename = "wordResults", default)]
    word_results: Vec<PaddleXWordResult>,
}

#[derive(Debug, Deserialize)]
struct PaddleXWordResult {
    text: String,
    #[serde(default)]
    confidence: f64,
    #[serde(rename = "wordRegion")]
    word_region: [[f64; 2]; 4],
}

#[async_trait::async_trait]
impl Backend for PaddleXBackend {
    async fn run(
        &self,
        image: &ImageInput,
        params: &RunParams,
    ) -> Result<ImageOutput, Error> {
        let file_part = image_part(image)?;

        let form = Form::new()
            .part("file", file_part)
            .text("returnWordBox", "true");

        let url = format!("{}/ocr", self.base_url.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| Error::connection(e.to_string(), "paddlex_ocr", true))?;

        let resp = check_response(resp, "PaddleX").await?;

        let parsed: PaddleXResponse = resp
            .json()
            .await
            .map_err(|e| Error::runtime(format!("PaddleX JSON parse error: {e}"), "paddlex_ocr", false))?;

        let threshold = params.confidence_threshold;
        let mut output = ImageOutput::new(image.source.derive());

        for ocr_result in &parsed.result.ocr_results {
            for word in &ocr_result.word_results {
                if word.confidence < threshold {
                    continue;
                }

                let polygon = Polygon {
                    vertices: word
                        .word_region
                        .iter()
                        .map(|[x, y]| Vertex::new(*x, *y))
                        .collect(),
                };
                let bbox = polygon.bounding_box();

                output.insert(ImageRegion {
                    text: word.text.clone(),
                    confidence: Some(word.confidence),
                    bbox,
                    polygon: Some(polygon),
                    level: Some(TextLevel::Word),
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
    fn parse_response() {
        let json = serde_json::json!({
            "result": {
                "ocrResults": [{
                    "text": "hello world",
                    "confidence": 0.95,
                    "textRegion": [[10, 20], [110, 20], [110, 40], [10, 40]],
                    "wordResults": [
                        {
                            "text": "hello",
                            "confidence": 0.96,
                            "wordRegion": [[10, 20], [60, 20], [60, 40], [10, 40]]
                        },
                        {
                            "text": "world",
                            "confidence": 0.94,
                            "wordRegion": [[65, 20], [110, 20], [110, 40], [65, 40]]
                        }
                    ]
                }]
            }
        });

        let resp: PaddleXResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.result.ocr_results.len(), 1);
        assert_eq!(resp.result.ocr_results[0].word_results.len(), 2);

        let word = &resp.result.ocr_results[0].word_results[0];
        assert_eq!(word.text, "hello");
        assert!((word.confidence - 0.96).abs() < 0.001);

        let polygon = Polygon {
            vertices: word
                .word_region
                .iter()
                .map(|[x, y]| Vertex::new(*x, *y))
                .collect(),
        };
        assert_eq!(polygon.vertices.len(), 4);
        assert!((polygon.vertices[0].x - 10.0).abs() < 0.01);
        assert!((polygon.vertices[0].y - 20.0).abs() < 0.01);

        let bbox = polygon.bounding_box();
        assert!((bbox.x - 10.0).abs() < 0.01);
        assert!((bbox.y - 20.0).abs() < 0.01);
        assert!((bbox.width - 50.0).abs() < 0.01);
        assert!((bbox.height - 20.0).abs() < 0.01);
    }

}

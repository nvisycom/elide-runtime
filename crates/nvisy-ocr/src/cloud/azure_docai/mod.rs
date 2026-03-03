//! [`OcrBackend`] implementation for Azure Document Intelligence.

use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use tokio::time::{Duration, sleep};

use nvisy_core::Error;
use nvisy_core::math::{BoundingBox, Polygon, Vertex};
use nvisy_ontology::location::TextLevel;

use crate::backend::{ImageInput, OcrBackend, OcrConfig, OcrRegion};

/// Poll interval when waiting for Azure analysis results.
const POLL_INTERVAL: Duration = Duration::from_millis(500);
/// Maximum number of poll attempts (500ms × 120 = 60s).
const MAX_POLL_ATTEMPTS: u32 = 120;

/// Remote OCR backend using Azure Document Intelligence.
///
/// Uses the async two-step flow: POST to start analysis, then poll GET
/// until results are available.
pub struct AzureDocaiBackend {
    client: ClientWithMiddleware,
    endpoint: String,
    api_key: String,
}

impl AzureDocaiBackend {
    /// Create a new backend with the given HTTP client, endpoint, and API key.
    ///
    /// The endpoint should be the Azure resource URL, e.g.
    /// `https://<resource>.cognitiveservices.azure.com`.
    pub fn new(
        client: ClientWithMiddleware,
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            client,
            endpoint: endpoint.into(),
            api_key: api_key.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnalyzeResponse {
    status: String,
    analyze_result: Option<AnalyzeResult>,
}

#[derive(Debug, Deserialize)]
struct AnalyzeResult {
    pages: Vec<AzurePage>,
}

#[derive(Debug, Deserialize)]
struct AzurePage {
    words: Vec<AzureWord>,
}

#[derive(Debug, Deserialize)]
struct AzureWord {
    content: String,
    #[serde(default)]
    confidence: f64,
    /// 8 floats: [x1, y1, x2, y2, x3, y3, x4, y4] forming a quadrilateral.
    #[serde(default)]
    polygon: Vec<f64>,
}

#[async_trait::async_trait]
impl OcrBackend for AzureDocaiBackend {
    async fn run(
        &self,
        image: &ImageInput,
        config: &OcrConfig,
    ) -> Result<Vec<OcrRegion>, Error> {
        let encoded = image.to_base64();
        let endpoint = self.endpoint.trim_end_matches('/');

        let submit_url = format!(
            "{endpoint}/documentintelligence/documentModels/prebuilt-read:analyze?api-version=2024-11-30"
        );

        let body = serde_json::json!({ "base64Source": encoded });

        let resp = self
            .client
            .post(&submit_url)
            .header("Ocp-Apim-Subscription-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                Error::runtime(
                    format!("Azure DocAI submit failed: {e}"),
                    "azure_docai_ocr",
                    false,
                )
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::runtime(
                format!("Azure DocAI submit returned {status}: {body}"),
                "azure_docai_ocr",
                false,
            ));
        }

        let result_url = resp
            .headers()
            .get("operation-location")
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .ok_or_else(|| {
                Error::runtime(
                    "Azure DocAI response missing Operation-Location header",
                    "azure_docai_ocr",
                    false,
                )
            })?;

        let mut attempts = 0u32;
        let analyze = loop {
            attempts += 1;
            if attempts > MAX_POLL_ATTEMPTS {
                return Err(Error::runtime(
                    "Azure DocAI analysis timed out after 60s",
                    "azure_docai_ocr",
                    false,
                ));
            }

            sleep(POLL_INTERVAL).await;

            let poll_resp = self
                .client
                .get(&result_url)
                .header("Ocp-Apim-Subscription-Key", &self.api_key)
                .send()
                .await
                .map_err(|e| {
                    Error::runtime(
                        format!("Azure DocAI poll failed: {e}"),
                        "azure_docai_ocr",
                        false,
                    )
                })?;

            if !poll_resp.status().is_success() {
                let status = poll_resp.status();
                let body = poll_resp.text().await.unwrap_or_default();
                return Err(Error::runtime(
                    format!("Azure DocAI poll returned {status}: {body}"),
                    "azure_docai_ocr",
                    false,
                ));
            }

            let parsed: AnalyzeResponse = poll_resp.json().await.map_err(|e| {
                Error::runtime(
                    format!("failed to parse Azure DocAI response: {e}"),
                    "azure_docai_ocr",
                    false,
                )
            })?;

            match parsed.status.as_str() {
                "succeeded" => break parsed,
                "failed" => {
                    return Err(Error::runtime(
                        "Azure DocAI analysis failed",
                        "azure_docai_ocr",
                        false,
                    ));
                }
                _ => continue,
            }
        };

        let threshold = config.confidence_threshold;
        let mut regions = Vec::new();

        let result = match &analyze.analyze_result {
            Some(r) => r,
            None => return Ok(regions),
        };

        for page in &result.pages {
            for word in &page.words {
                if word.confidence < threshold {
                    continue;
                }

                let polygon = if word.polygon.len() == 8 {
                    Some(Polygon {
                        vertices: word
                            .polygon
                            .chunks_exact(2)
                            .map(|pair| Vertex::new(pair[0], pair[1]))
                            .collect(),
                    })
                } else {
                    None
                };

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
                    text: word.content.clone(),
                    confidence: word.confidence,
                    bbox,
                    polygon,
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
            "status": "succeeded",
            "analyzeResult": {
                "pages": [{
                    "words": [
                        {
                            "content": "Hello",
                            "confidence": 0.99,
                            "polygon": [10.0, 20.0, 50.0, 20.0, 50.0, 40.0, 10.0, 40.0]
                        },
                        {
                            "content": "World",
                            "confidence": 0.95,
                            "polygon": [60.0, 20.0, 110.0, 20.0, 110.0, 40.0, 60.0, 40.0]
                        }
                    ]
                }]
            }
        });

        let resp: AnalyzeResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.status, "succeeded");

        let result = resp.analyze_result.as_ref().unwrap();
        assert_eq!(result.pages.len(), 1);
        assert_eq!(result.pages[0].words.len(), 2);

        let word = &result.pages[0].words[0];
        assert_eq!(word.content, "Hello");
        assert!((word.confidence - 0.99).abs() < 0.001);
        assert_eq!(word.polygon.len(), 8);
    }

    #[test]
    fn polygon_from_8_floats() {
        let word = AzureWord {
            content: "test".into(),
            confidence: 0.9,
            polygon: vec![0.0, 0.0, 10.0, 0.0, 10.0, 5.0, 0.0, 5.0],
        };

        let polygon = Polygon {
            vertices: word
                .polygon
                .chunks_exact(2)
                .map(|pair| Vertex::new(pair[0], pair[1]))
                .collect(),
        };

        assert_eq!(polygon.vertices.len(), 4);
        let bbox = polygon.bounding_box();
        assert!((bbox.width - 10.0).abs() < 0.001);
        assert!((bbox.height - 5.0).abs() < 0.001);
    }

    #[test]
    fn filters_below_threshold() {
        let json = serde_json::json!({
            "status": "succeeded",
            "analyzeResult": {
                "pages": [{
                    "words": [
                        {
                            "content": "low",
                            "confidence": 0.2,
                            "polygon": [0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0]
                        },
                        {
                            "content": "high",
                            "confidence": 0.95,
                            "polygon": [2.0, 0.0, 3.0, 0.0, 3.0, 1.0, 2.0, 1.0]
                        }
                    ]
                }]
            }
        });

        let resp: AnalyzeResponse = serde_json::from_value(json).unwrap();
        let threshold = 0.5;

        let words: Vec<_> = resp
            .analyze_result
            .as_ref()
            .unwrap()
            .pages[0]
            .words
            .iter()
            .filter(|w| w.confidence >= threshold)
            .collect();

        assert_eq!(words.len(), 1);
        assert_eq!(words[0].content, "high");
    }
}

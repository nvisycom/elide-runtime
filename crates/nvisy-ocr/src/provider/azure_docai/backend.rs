//! [`Backend`] implementation for Azure Document Intelligence.
//!
//! [`Backend`]: crate::Backend

use std::fmt;

use nvisy_core::Error;
use nvisy_core::math::{Polygon, Vertex};
use nvisy_http::HttpClient;
use serde::Deserialize;
use tokio::time::{Duration, sleep};

use super::AzureDocaiParams;
use crate::backend::{
    Backend, ImageInput, ImageOutput, ImageRegion, RunParams, TextLevel, check_response,
};

/// [`Backend`] implementation for Azure Document Intelligence.
///
/// Uses the async two-step flow: POST to start analysis, then poll GET
/// until results are available.
///
/// [`Backend`]: crate::Backend
pub struct AzureDocaiBackend {
    client: HttpClient,
    endpoint: String,
    api_key: String,
    api_version: String,
    poll_interval: Duration,
    max_poll_attempts: u32,
}

impl fmt::Debug for AzureDocaiBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AzureDocaiBackend")
            .field("endpoint", &self.endpoint)
            .field("api_key", &"***")
            .field("api_version", &self.api_version)
            .finish()
    }
}

impl AzureDocaiBackend {
    /// Create a new backend with default HTTP configuration.
    pub fn new(params: AzureDocaiParams) -> Self {
        Self::with_client(HttpClient::default(), params)
    }

    /// Create a new backend with a pre-configured HTTP client.
    pub fn with_client(client: HttpClient, params: AzureDocaiParams) -> Self {
        let poll_interval = Duration::from_millis(params.poll_interval_ms.unwrap_or(500));
        let max_poll_attempts = params.max_poll_attempts.unwrap_or(120);
        let api_version = params
            .api_version
            .unwrap_or_else(|| "2024-11-30".to_owned());

        Self {
            client,
            endpoint: params.endpoint,
            api_key: params.api_key,
            api_version,
            poll_interval,
            max_poll_attempts,
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
impl Backend for AzureDocaiBackend {
    async fn run(&self, image: &ImageInput, params: &RunParams) -> Result<ImageOutput, Error> {
        let encoded = image.to_base64();
        let endpoint = self.endpoint.trim_end_matches('/');

        let submit_url = format!(
            "{endpoint}/documentintelligence/documentModels/prebuilt-read:analyze?api-version={}",
            self.api_version
        );

        let body = serde_json::json!({ "base64Source": encoded });

        let resp = self
            .client
            .post(&submit_url)
            .header("Ocp-Apim-Subscription-Key", &*self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::connection(e.to_string(), "azure_docai_ocr", true))?;

        let resp = check_response(resp, "Azure DocAI submit").await?;

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

        if !result_url.starts_with(endpoint) {
            return Err(Error::runtime(
                format!("Azure DocAI returned unexpected Operation-Location host: {result_url}"),
                "azure_docai_ocr",
                false,
            ));
        }

        let mut attempts = 0u32;
        let analyze = loop {
            if attempts >= self.max_poll_attempts {
                return Err(Error::runtime(
                    format!(
                        "Azure DocAI analysis timed out after {} attempts",
                        self.max_poll_attempts
                    ),
                    "azure_docai_ocr",
                    false,
                ));
            }
            attempts += 1;

            let poll_resp = self
                .client
                .get(&result_url)
                .header("Ocp-Apim-Subscription-Key", &*self.api_key)
                .send()
                .await
                .map_err(|e| Error::connection(e.to_string(), "azure_docai_ocr", true))?;

            let poll_resp = check_response(poll_resp, "Azure DocAI poll").await?;

            let parsed: AnalyzeResponse = poll_resp.json().await.map_err(|e| {
                Error::runtime(
                    format!("Azure DocAI JSON parse error: {e}"),
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
                _ => {
                    sleep(self.poll_interval).await;
                }
            }
        };

        let threshold = params.confidence_threshold;
        let mut output = ImageOutput::new(image.source.derive());

        let result = match &analyze.analyze_result {
            Some(r) => r,
            None => return Ok(output),
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
                    .unwrap_or_default();

                output.insert(ImageRegion {
                    text: word.content.clone(),
                    confidence: Some(word.confidence),
                    bbox,
                    polygon,
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
}

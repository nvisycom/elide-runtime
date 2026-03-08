//! [`Backend`] implementation for AWS Textract.
//!
//! [`Backend`]: crate::Backend

use std::fmt;

use hmac::{Hmac, Mac};
use nvisy_core::Error;
use nvisy_core::math::{BoundingBox, Polygon, Vertex};
use nvisy_http::HttpClient;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::AwsTextractParams;
use crate::backend::{
    Backend, ImageInput, ImageOutput, ImageRegion, RunParams, TextLevel, check_response,
};

/// [`Backend`] implementation for AWS Textract.
///
/// Sends images as base64-encoded JSON to the `DetectDocumentText` action
/// with inline SigV4 request signing.
///
/// [`Backend`]: crate::Backend
pub struct AwsTextractBackend {
    client: HttpClient,
    access_key: String,
    secret_key: String,
    region: String,
}

impl fmt::Debug for AwsTextractBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AwsTextractBackend")
            .field("access_key", &"***")
            .field("secret_key", &"***")
            .field("region", &self.region)
            .finish()
    }
}

impl AwsTextractBackend {
    /// Create a new backend with default HTTP configuration.
    pub fn new(params: AwsTextractParams) -> Self {
        Self::with_client(HttpClient::default(), params)
    }

    /// Create a new backend with a pre-configured HTTP client.
    pub fn with_client(client: HttpClient, params: AwsTextractParams) -> Self {
        Self {
            client,
            access_key: params.access_key,
            secret_key: params.secret_key,
            region: params.region,
        }
    }
}

type HmacSha256 = Hmac<Sha256>;

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

struct SigV4Params<'a> {
    access_key: &'a str,
    secret_key: &'a str,
    region: &'a str,
    service: &'a str,
    date_stamp: &'a str,
    datetime: &'a str,
    host: &'a str,
    payload: &'a [u8],
}

fn sign_request(p: &SigV4Params<'_>) -> String {
    let payload_hash = sha256_hex(p.payload);

    let canonical_request = format!(
        "POST\n/\n\ncontent-type:application/x-amz-json-1.1\nhost:{}\nx-amz-date:{}\nx-amz-target:Textract.DetectDocumentText\n\ncontent-type;host;x-amz-date;x-amz-target\n{payload_hash}",
        p.host, p.datetime
    );

    let credential_scope = format!("{}/{}/{}/aws4_request", p.date_stamp, p.region, p.service);

    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{credential_scope}\n{}",
        p.datetime,
        sha256_hex(canonical_request.as_bytes())
    );

    let k_date = hmac_sha256(
        format!("AWS4{}", p.secret_key).as_bytes(),
        p.date_stamp.as_bytes(),
    );
    let k_region = hmac_sha256(&k_date, p.region.as_bytes());
    let k_service = hmac_sha256(&k_region, p.service.as_bytes());
    let k_signing = hmac_sha256(&k_service, b"aws4_request");
    let signature = hmac_sha256(&k_signing, string_to_sign.as_bytes());
    let signature_hex: String = signature.iter().map(|b| format!("{b:02x}")).collect();

    format!(
        "AWS4-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders=content-type;host;x-amz-date;x-amz-target, Signature={signature_hex}",
        p.access_key
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TextractResponse {
    blocks: Vec<TextractBlock>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TextractBlock {
    block_type: String,
    text: Option<String>,
    confidence: Option<f64>,
    geometry: Option<TextractGeometry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TextractGeometry {
    bounding_box: Option<TextractBBox>,
    polygon: Option<Vec<TextractPoint>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TextractBBox {
    width: f64,
    height: f64,
    left: f64,
    top: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TextractPoint {
    x: f64,
    y: f64,
}

#[async_trait::async_trait]
impl Backend for AwsTextractBackend {
    async fn run(&self, image: &ImageInput, params: &RunParams) -> Result<ImageOutput, Error> {
        let encoded = image.to_base64();

        let body = serde_json::json!({
            "Document": {
                "Bytes": encoded
            }
        });
        let payload = serde_json::to_vec(&body).map_err(|e| {
            Error::runtime(
                format!("failed to serialize Textract request: {e}"),
                "aws_textract_ocr",
                false,
            )
        })?;

        let host = format!("textract.{}.amazonaws.com", self.region);
        let url = format!("https://{host}/");

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| {
                Error::runtime(
                    "system clock is before UNIX epoch",
                    "aws_textract_ocr",
                    false,
                )
            })?;
        let secs = now.as_secs();
        let datetime = format_datetime(secs);
        let date_stamp = &datetime[..8];

        let authorization = sign_request(&SigV4Params {
            access_key: &self.access_key,
            secret_key: &self.secret_key,
            region: &self.region,
            service: "textract",
            date_stamp,
            datetime: &datetime,
            host: &host,
            payload: &payload,
        });

        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/x-amz-json-1.1")
            .header("X-Amz-Target", "Textract.DetectDocumentText")
            .header("X-Amz-Date", &*datetime)
            .header("Authorization", &*authorization)
            .body(payload)
            .send()
            .await
            .map_err(|e| Error::connection(e.to_string(), "aws_textract_ocr", true))?;

        let resp = check_response(resp, "Textract").await?;

        let parsed: TextractResponse = resp.json().await.map_err(|e| {
            Error::runtime(
                format!("Textract JSON parse error: {e}"),
                "aws_textract_ocr",
                false,
            )
        })?;

        let threshold = params.confidence_threshold;
        let mut output = ImageOutput::new(image.source.derive());

        for block in &parsed.blocks {
            if block.block_type != "WORD" {
                continue;
            }

            let text = match &block.text {
                Some(t) => t.clone(),
                None => continue,
            };

            // Textract returns confidence as 0–100; normalise to 0–1.
            let confidence = block.confidence.unwrap_or(0.0) / 100.0;

            if confidence < threshold {
                continue;
            }

            let (bbox, polygon) = match &block.geometry {
                Some(geom) => {
                    let bbox = geom
                        .bounding_box
                        .as_ref()
                        .map(|b| BoundingBox {
                            x: b.left,
                            y: b.top,
                            width: b.width,
                            height: b.height,
                        })
                        .unwrap_or_default();

                    let polygon = geom.polygon.as_ref().map(|pts| Polygon {
                        vertices: pts.iter().map(|p| Vertex::new(p.x, p.y)).collect(),
                    });

                    (bbox, polygon)
                }
                None => (BoundingBox::default(), None),
            };

            output.insert(ImageRegion {
                text,
                confidence: Some(confidence),
                bbox,
                polygon,
                level: Some(TextLevel::Word),
            });
        }

        Ok(output)
    }
}

/// Format epoch seconds as `YYYYMMDDTHHMMSSZ` for AWS SigV4.
fn format_datetime(epoch_secs: u64) -> String {
    let days_since_epoch = epoch_secs / 86400;
    let time_of_day = epoch_secs % 86400;

    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Civil date from days since 1970-01-01 (algorithm from Howard Hinnant).
    let z = days_since_epoch as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}{m:02}{d:02}T{hours:02}{minutes:02}{seconds:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_response() {
        let json = serde_json::json!({
            "Blocks": [
                {
                    "BlockType": "PAGE",
                    "Geometry": {
                        "BoundingBox": { "Width": 1.0, "Height": 1.0, "Left": 0.0, "Top": 0.0 }
                    }
                },
                {
                    "BlockType": "WORD",
                    "Text": "hello",
                    "Confidence": 99.5,
                    "Geometry": {
                        "BoundingBox": {
                            "Width": 0.2,
                            "Height": 0.05,
                            "Left": 0.1,
                            "Top": 0.3
                        },
                        "Polygon": [
                            { "X": 0.1, "Y": 0.3 },
                            { "X": 0.3, "Y": 0.3 },
                            { "X": 0.3, "Y": 0.35 },
                            { "X": 0.1, "Y": 0.35 }
                        ]
                    }
                }
            ]
        });

        let resp: TextractResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.blocks.len(), 2);

        let word = &resp.blocks[1];
        assert_eq!(word.block_type, "WORD");
        assert_eq!(word.text.as_deref(), Some("hello"));
        assert!((word.confidence.unwrap() - 99.5).abs() < 0.01);

        let geom = word.geometry.as_ref().unwrap();
        let bbox = geom.bounding_box.as_ref().unwrap();
        assert!((bbox.left - 0.1).abs() < 0.001);
        assert!((bbox.top - 0.3).abs() < 0.001);
        assert!((bbox.width - 0.2).abs() < 0.001);
    }

    #[test]
    fn format_datetime_known_epoch() {
        // 2024-01-15T11:30:45Z = 1705318245 seconds since epoch
        let result = format_datetime(1705318245);
        assert_eq!(result, "20240115T113045Z");
    }

    #[test]
    fn sigv4_signing_produces_output() {
        let auth = sign_request(&SigV4Params {
            access_key: "AKIAIOSFODNN7EXAMPLE",
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            region: "us-east-1",
            service: "textract",
            date_stamp: "20240115",
            datetime: "20240115T120000Z",
            host: "textract.us-east-1.amazonaws.com",
            payload: b"{}",
        });
        assert!(auth.starts_with("AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/"));
        assert!(auth.contains("SignedHeaders=content-type;host;x-amz-date;x-amz-target"));
    }
}

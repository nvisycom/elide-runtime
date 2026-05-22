//! [`Backend`] implementation for AWS Textract.
//!
//! [`Backend`]: crate::Backend

use std::collections::HashMap;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, KeyInit, Mac};
use nvisy_core::{Error, Result};
use nvisy_http::{HttpClient, HttpConfig, RequestBuilderExt};
use nvisy_ontology::artifacts::{Block, BlockKind, Line, Page, Word};
use nvisy_ontology::primitive::{BoundingBox, Polygon, Vertex};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::AwsTextractParams;
use crate::backend::{Backend, ImageInput, ImageOutput, RunParams};

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
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn new(params: AwsTextractParams) -> Result<Self> {
        Ok(Self::with_client(
            HttpClient::new(&HttpConfig::default())?,
            params,
        ))
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
    id: Option<String>,
    text: Option<String>,
    confidence: Option<f64>,
    geometry: Option<TextractGeometry>,
    relationships: Option<Vec<TextractRelationship>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TextractRelationship {
    r#type: String,
    ids: Vec<String>,
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

fn extract_geometry(geom: Option<&TextractGeometry>) -> (BoundingBox, Option<Polygon>) {
    match geom {
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
    }
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

        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
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

        let parsed: TextractResponse = self
            .client
            .post(&url)
            .header("Content-Type", "application/x-amz-json-1.1")
            .header("X-Amz-Target", "Textract.DetectDocumentText")
            .header("X-Amz-Date", &*datetime)
            .header("Authorization", &*authorization)
            .body(payload)
            .send_and_parse("aws_textract_ocr")
            .await?;

        let threshold = params.confidence_threshold;

        // Index blocks by ID for relationship lookups.
        let block_map: HashMap<&str, &TextractBlock> = parsed
            .blocks
            .iter()
            .filter_map(|b| b.id.as_deref().map(|id| (id, b)))
            .collect();

        fn child_ids(block: &TextractBlock) -> Vec<&str> {
            block
                .relationships
                .as_ref()
                .into_iter()
                .flatten()
                .filter(|r| r.r#type == "CHILD")
                .flat_map(|r| r.ids.iter().map(|s| s.as_str()))
                .collect()
        }

        let mut output = ImageOutput::new(image.source.derive());

        // Iterate PAGE blocks; build LINE→WORD tree from relationships.
        let mut page_number = 0u32;
        for block in &parsed.blocks {
            if block.block_type != "PAGE" {
                continue;
            }
            page_number += 1;

            let line_ids = child_ids(block);
            let mut lines = Vec::new();

            for line_id in &line_ids {
                let line_block = match block_map.get(line_id) {
                    Some(b) if b.block_type == "LINE" => b,
                    _ => continue,
                };

                let word_ids = child_ids(line_block);
                let mut words = Vec::new();

                for word_id in &word_ids {
                    let word_block = match block_map.get(word_id) {
                        Some(b) if b.block_type == "WORD" => b,
                        _ => continue,
                    };

                    let text = match &word_block.text {
                        Some(t) => t.clone(),
                        None => continue,
                    };

                    // Textract returns confidence as 0–100; normalise to 0–1.
                    let confidence = word_block.confidence.unwrap_or(0.0) / 100.0;
                    if confidence < threshold {
                        continue;
                    }

                    let (bbox, polygon) = extract_geometry(word_block.geometry.as_ref());

                    words.push(Word {
                        text,
                        confidence: Some(confidence),
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
                let line_confidence = line_block.confidence.map(|c| c / 100.0);
                let (line_bbox, line_polygon) = extract_geometry(line_block.geometry.as_ref());

                lines.push(Line {
                    text: line_text,
                    confidence: line_confidence,
                    bbox: line_bbox,
                    polygon: line_polygon,
                    words,
                });
            }

            let block_text = lines
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let (page_bbox, _) = extract_geometry(block.geometry.as_ref());

            output.pages.push(Page {
                page_number,
                width: Some(page_bbox.width),
                height: Some(page_bbox.height),
                blocks: vec![Block {
                    text: block_text,
                    confidence: None,
                    bbox: page_bbox,
                    polygon: None,
                    kind: BlockKind::Text,
                    lines,
                }],
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
    fn build_hierarchy_from_relationships() {
        let json = serde_json::json!({
            "Blocks": [
                {
                    "BlockType": "PAGE",
                    "Id": "page-1",
                    "Geometry": {
                        "BoundingBox": { "Width": 1.0, "Height": 1.0, "Left": 0.0, "Top": 0.0 }
                    },
                    "Relationships": [{
                        "Type": "CHILD",
                        "Ids": ["line-1"]
                    }]
                },
                {
                    "BlockType": "LINE",
                    "Id": "line-1",
                    "Text": "hello world",
                    "Confidence": 98.0,
                    "Geometry": {
                        "BoundingBox": { "Width": 0.5, "Height": 0.05, "Left": 0.1, "Top": 0.3 }
                    },
                    "Relationships": [{
                        "Type": "CHILD",
                        "Ids": ["word-1", "word-2"]
                    }]
                },
                {
                    "BlockType": "WORD",
                    "Id": "word-1",
                    "Text": "hello",
                    "Confidence": 99.0,
                    "Geometry": {
                        "BoundingBox": { "Width": 0.2, "Height": 0.05, "Left": 0.1, "Top": 0.3 }
                    }
                },
                {
                    "BlockType": "WORD",
                    "Id": "word-2",
                    "Text": "world",
                    "Confidence": 97.0,
                    "Geometry": {
                        "BoundingBox": { "Width": 0.2, "Height": 0.05, "Left": 0.35, "Top": 0.3 }
                    }
                }
            ]
        });

        let resp: TextractResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.blocks.len(), 4);

        // Verify the relationship structure.
        let page = &resp.blocks[0];
        assert_eq!(page.block_type, "PAGE");
        let rels = page.relationships.as_ref().unwrap();
        assert_eq!(rels[0].ids, vec!["line-1"]);

        let line = &resp.blocks[1];
        assert_eq!(line.block_type, "LINE");
        let rels = line.relationships.as_ref().unwrap();
        assert_eq!(rels[0].ids, vec!["word-1", "word-2"]);
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

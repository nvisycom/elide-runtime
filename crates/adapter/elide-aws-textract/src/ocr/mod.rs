//! [`TextractOcr`]: an [`OcrBackend`] backed by AWS Textract.
//!
//! [`OcrBackend`]: elide_image::ocr::OcrBackend

mod response;

use async_trait::async_trait;
use aws_sdk_textract::Client;
use aws_sdk_textract::primitives::Blob;
use aws_sdk_textract::types::Document;
use elide_core::Result;
use elide_core::entity::audit::ModelEvent;
use elide_image::ocr::{OcrBackend, OcrRequest, OcrResponse};
use hipstr::HipStr;

use crate::error::TextractError;

/// An [`OcrBackend`] backed by AWS Textract's `DetectDocumentText`.
///
/// # Where the image goes
///
/// **This sends document images to a third party**, which for a redaction
/// pipeline is the un-redacted original. Textract's default data posture
/// is the weakest of the hosted OCR providers — AWS may use content for
/// service improvement or model training, and may store it outside the
/// region in use — so an AWS Organizations opt-out policy is day-one
/// work, not a setting. See the crate README.
#[derive(Debug, Clone)]
pub struct TextractOcr {
    client: Client,
}

impl TextractOcr {
    /// Build from a configured [`Client`].
    ///
    /// The client carries the region, credentials and retry policy. The
    /// SDK is re-exported as [`textract`], so a caller configuring one
    /// does not need to depend on it separately.
    ///
    /// Give it a bounded operation timeout: the SDK leaves both the
    /// operation and per-attempt timeouts unset, and [`recognize`] is a
    /// single `await` with no deadline of its own.
    ///
    /// [`recognize`]: OcrBackend::recognize
    ///
    /// [`textract`]: crate::textract
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl OcrBackend for TextractOcr {
    fn provenance(&self) -> ModelEvent {
        ModelEvent {
            name: HipStr::borrowed("aws/textract"),
            version: None,
            contextual: false,
        }
    }

    async fn recognize(&self, request: OcrRequest<'_>) -> Result<OcrResponse> {
        // `DetectDocumentText` rather than `AnalyzeDocument`: this backend
        // wants text and geometry, and the analysis features (forms,
        // tables, queries) cost more per page for results elide's layout
        // model has nowhere to put.
        let document = Document::builder()
            .bytes(Blob::new(request.image.to_vec()))
            .build();

        let response = self
            .client
            .detect_document_text()
            .document(document)
            .send()
            .await
            .map_err(|err| TextractError::from_sdk(&err))?;

        let blocks = response
            .blocks
            .ok_or_else(|| TextractError::Protocol("response carried no blocks".to_owned()))?;

        Ok(self::response::decode(&blocks, request.dimensions))
    }
}

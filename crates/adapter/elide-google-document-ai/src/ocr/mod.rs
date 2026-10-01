//! [`DocumentAiOcr`]: a [`Backend`] backed by Google Cloud Document AI.
//!
//! [`Backend`]: elide_core::backend::Backend

mod response;

use async_trait::async_trait;
use elide_core::Result;
use elide_core::backend::Backend;
use elide_core::entity::audit::ModelEvent;
use elide_image::ocr::{OcrRequest, OcrResponse};
use google_cloud_documentai_v1::client::DocumentProcessorService;
use google_cloud_documentai_v1::model::RawDocument;
use hipstr::HipStr;

use crate::error::DocumentAiBackendError;

/// A [`Backend`] backed by Google Cloud Document AI.
#[derive(Clone)]
pub struct DocumentAiOcr {
    client: DocumentProcessorService,
    processor: String,
}

impl DocumentAiOcr {
    /// Build from a client and the full resource name of the processor to
    /// call, `projects/{p}/locations/{l}/processors/{id}`.
    #[must_use]
    pub fn new(client: DocumentProcessorService, processor: impl Into<String>) -> Self {
        Self {
            client,
            processor: processor.into(),
        }
    }
}

#[async_trait]
impl Backend for DocumentAiOcr {
    type Request<'a> = OcrRequest<'a>;
    type Response = OcrResponse;

    fn provenance(&self) -> ModelEvent {
        ModelEvent {
            name: HipStr::borrowed("google/document-ai"),
            version: None,
            contextual: false,
        }
    }

    async fn call(&self, request: OcrRequest<'_>) -> Result<OcrResponse> {
        // Document AI dispatches on the MIME type and answers
        // `INVALID_ARGUMENT` for one outside its supported list, so the
        // format the caller resolved at ingestion is named explicitly
        // rather than left to be sniffed.
        let raw = RawDocument::new()
            .set_content(request.image.to_vec())
            .set_mime_type(request.format.mime_type());

        let response = self
            .client
            .process_document()
            .set_name(&self.processor)
            .set_raw_document(raw)
            .send()
            .await
            .map_err(DocumentAiBackendError::from)?;

        let document = response.document.ok_or_else(|| {
            DocumentAiBackendError::Protocol("response carried no document".into())
        })?;

        Ok(self::response::decode(&document))
    }
}

//! [`OcrExtractor`]: type-erased OCR extractor wrapping any
//! [`OcrBackend`].

use std::fmt;
use std::sync::Arc;

use nvisy_core::entity::ModelProvenance;
use nvisy_core::extraction::{Extractor as CoreExtractor, ExtractorOutput, Span};
use nvisy_core::modality::{Image, ImageExtraction};
use nvisy_core::{Error, Result};

use crate::backend::{OcrBackend, OcrRequest, OcrResponse};

const TARGET: &str = "nvisy_ocr::extraction";

/// Type-erased OCR extractor wrapping any [`OcrBackend`]
/// implementation.
///
/// Owns an `Arc<dyn OcrBackend>` and forwards OCR requests to it,
/// providing a concrete, object-safe entry point without generics
/// at every call site. The extractor is `Clone` — cloning shares
/// the backend.
#[derive(Clone)]
pub struct OcrExtractor {
    backend: Arc<dyn OcrBackend>,
}

impl fmt::Debug for OcrExtractor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OcrExtractor").finish_non_exhaustive()
    }
}

impl OcrExtractor {
    /// Create a new extractor from any [`OcrBackend`] implementation.
    pub fn new(backend: impl OcrBackend) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }

    /// Provenance of the wrapped backend, forwarded from
    /// [`OcrBackend::provenance`].
    pub fn provenance(&self) -> ModelProvenance {
        self.backend.provenance()
    }

    /// Extract OCR blocks from a single image.
    #[tracing::instrument(skip_all, fields(image_bytes = request.image.len()))]
    pub async fn extract(&self, request: OcrRequest<'_>) -> Result<OcrResponse, Error> {
        let response = self.backend.extract(request).await?;
        tracing::debug!(
            target: TARGET,
            spans = response.blocks.iter().map(|b| b.spans.len()).sum::<usize>(),
            "ocr complete",
        );
        Ok(response)
    }

    /// Extract OCR blocks from multiple images, concatenating the
    /// per-image blocks. See [`OcrBackend::extract_batch`] for the
    /// same-source assumption.
    #[tracing::instrument(skip_all, fields(count = requests.len()))]
    pub async fn extract_batch(&self, requests: &[OcrRequest<'_>]) -> Result<OcrResponse, Error> {
        let response = self.backend.extract_batch(requests).await?;
        tracing::debug!(
            target: TARGET,
            spans = response.blocks.iter().map(|b| b.spans.len()).sum::<usize>(),
            "batch ocr complete",
        );
        Ok(response)
    }
}

/// Bridge `nvisy_ocr::OcrExtractor` into the core-side
/// [`Extractor<Image>`] surface. The extractor's output is the
/// backend-shaped [`OcrResponse`]; consumers translate that into
/// per-document `Block<Image>` values.
///
/// [`Extractor<Image>`]: nvisy_core::extraction::Extractor
#[async_trait::async_trait]
impl CoreExtractor<Image> for OcrExtractor {
    type Output = OcrResponse;

    async fn extract(&self, span: &Span<Image>) -> Result<ExtractorOutput<Image, Self::Output>> {
        let mut request = OcrRequest::new(&span.data.bytes);
        if let Some(ref lang) = span.language {
            request = request.with_language(lang);
        }
        if let Some(corr_id) = span.correlation_id {
            request = request.with_correlation_id(corr_id);
        }
        let response = self.backend.extract(request).await?;
        Ok(ExtractorOutput::new(
            response,
            ImageExtraction::Ocr(self.provenance()),
        ))
    }
}

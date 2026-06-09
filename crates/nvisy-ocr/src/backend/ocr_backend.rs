//! [`OcrBackend`]: the OCR backend contract.

use async_trait::async_trait;
use bytes::Bytes;
use nvisy_core::Error;
use nvisy_core::entity::ModelProvenance;
use nvisy_core::primitive::LanguageTag;
use uuid::Uuid;

use crate::types::RawOcrBlock;

/// One per-call OCR request handed to an [`OcrBackend`].
///
/// Bundles the image bytes with advisory hints (language,
/// correlation id). Borrowed (`OcrRequest<'a>`) so call sites that
/// already own the underlying values hand them through without
/// cloning.
#[derive(Debug, Clone)]
pub struct OcrRequest<'a> {
    /// Raw image bytes. Returned span / region coordinates refer
    /// back into the pixel space of this image.
    pub image: &'a Bytes,
    /// Caller-asserted language. Multilingual OCR engines may
    /// ignore it; engines with a per-language model variant use it
    /// to pick the right one.
    pub language: Option<&'a LanguageTag>,
    /// Per-call correlation id propagated to remote backends (as
    /// the `x-request-id` header on the Bento backend). When
    /// `None`, transports that need a request id generate a
    /// UUIDv7 themselves so every request is traceable.
    pub correlation_id: Option<Uuid>,
}

impl<'a> OcrRequest<'a> {
    /// Construct a request with no advisory hints set.
    pub fn new(image: &'a Bytes) -> Self {
        Self {
            image,
            language: None,
            correlation_id: None,
        }
    }

    /// Builder-style setter for the language hint.
    #[must_use]
    pub fn with_language(mut self, language: &'a LanguageTag) -> Self {
        self.language = Some(language);
        self
    }

    /// Builder-style setter for the correlation id.
    #[must_use]
    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }
}

/// One per-call OCR response from an [`OcrBackend`].
///
/// Wraps the raw blocks the backend produced. Pre-normalization:
/// the extractor wraps each block into the per-document block
/// shape and stamps provenance onto the surrounding metadata.
#[derive(Debug, Clone, Default)]
pub struct OcrResponse {
    /// Blocks predicted for the request's image, in backend order.
    pub blocks: Vec<RawOcrBlock>,
}

impl OcrResponse {
    /// Construct a response from raw blocks.
    #[must_use]
    pub fn new(blocks: Vec<RawOcrBlock>) -> Self {
        Self { blocks }
    }
}

/// Per-call OCR backend.
///
/// Implementations send an image to an OCR service and return an
/// [`OcrResponse`] — one [`RawOcrBlock`] per page or text region,
/// with per-word spans populated and bounding boxes preserved on
/// each location.
///
/// Backends are source-agnostic: they take bytes + hints and
/// return shape. Wrapping the blocks into a per-document container
/// happens at the extractor boundary.
///
/// Confidence values **must** be normalised to `0.0..=1.0` before
/// being placed on spans. Backends whose upstream API uses a
/// different scale are responsible for converting.
///
/// Implementors **must** provide [`extract`]. The default
/// [`extract_batch`] impl dispatches the inputs concurrently via
/// `futures::join_all` and concatenates the per-image responses.
/// Backends with a native batch API (such as a single network
/// round-trip) override it to merge server-side.
///
/// Batch entries **are assumed to come from the same source** —
/// the typical caller is a multi-page document split into
/// per-page images, so the per-request hints apply uniformly and
/// the per-image blocks can be merged without further bookkeeping.
/// Mixed-source inputs should be issued as separate batches.
///
/// Per-image page numbering is returned as-is — if the caller
/// needs them rebased onto a containing document, the caller knows
/// the per-image page offsets and is responsible for that rebase.
///
/// Object-safe: extractors hold `Arc<dyn OcrBackend>` and dispatch
/// per call.
///
/// [`extract`]: Self::extract
/// [`extract_batch`]: Self::extract_batch
#[async_trait]
pub trait OcrBackend: Send + Sync + 'static {
    /// Backend identity (model / service name + provenance kind).
    ///
    /// The document-side extraction phase reads this after
    /// recognition runs and stamps it into
    /// [`ImageExtraction::Ocr`] on the document's metadata, so the
    /// audit records *which* OCR pass produced the document.
    ///
    /// [`ImageExtraction::Ocr`]: nvisy_core::modality::ImageExtraction::Ocr
    fn provenance(&self) -> ModelProvenance;

    /// Extract OCR blocks for `request`.
    ///
    /// # Errors
    ///
    /// Returns the underlying transport / parse / inference error.
    async fn extract(&self, request: OcrRequest<'_>) -> Result<OcrResponse, Error>;

    /// Batched extract. Defaults to a concurrent fan-out via
    /// `futures::join_all` and concatenates the per-image
    /// responses into a single [`OcrResponse`]; backends with a
    /// native batch API override it.
    ///
    /// `requests` is assumed to be slices of the same source (see
    /// the trait-level docs).
    ///
    /// # Errors
    ///
    /// Returns the first error encountered.
    async fn extract_batch(&self, requests: &[OcrRequest<'_>]) -> Result<OcrResponse, Error> {
        let pending: Vec<_> = requests.iter().map(|req| self.extract(req.clone())).collect();
        let results: Vec<Result<OcrResponse, Error>> = futures::future::join_all(pending).await;
        let mut merged: Vec<RawOcrBlock> = Vec::new();
        for r in results {
            merged.extend(r?.blocks);
        }
        Ok(OcrResponse::new(merged))
    }
}

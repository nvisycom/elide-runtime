//! OCR text extraction from images.
//!
//! Extracts text regions from image spans by delegating to the
//! nvisy-ocr [`OcrEngine`]'s text recognition pipeline.
//!
//! [`OcrEngine`]: nvisy_ocr::OcrEngine

use nvisy_codec::Span;
use nvisy_codec::handler::ImageData;
use nvisy_core::Result;
use nvisy_ocr::{ImageFormat, ImageInput, ImageOutput, OcrEngine, RunParams};

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::ocr";

/// OCR text-extraction operation: thin adapter around [`OcrEngine`].
///
/// [`OcrEngine`]: nvisy_ocr::OcrEngine
pub struct Ocr {
    engine: OcrEngine,
    params: RunParams,
}

impl Ocr {
    /// Create a new OCR operation from a pre-built engine.
    pub fn new(engine: OcrEngine, params: RunParams) -> Self {
        Self { engine, params }
    }

    fn to_image_input(span: &Span<(), ImageData>) -> Result<ImageInput> {
        let png_bytes = span.data.encode_png()?;
        Ok(ImageInput::with_source(
            span.source,
            png_bytes,
            ImageFormat::Png,
        ))
    }

    async fn extract(&self, spans: Vec<Span<(), ImageData>>) -> Result<Vec<ImageOutput>> {
        if spans.is_empty() {
            tracing::debug!(target: TARGET, "no spans to process");
            return Ok(Vec::new());
        }
        tracing::debug!(target: TARGET, span_count = spans.len(), "extracting text");

        let images = spans
            .iter()
            .map(Self::to_image_input)
            .collect::<Result<Vec<_>>>()?;

        self.engine.run_batch(&images, &self.params).await
    }
}

impl Operation for Ocr {
    type Input = ParallelContext<Vec<Span<(), ImageData>>>;
    type Output = ParallelContext<Vec<ImageOutput>>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|spans| self.extract(spans)).await
    }
}

//! OCR text extraction from images.
//!
//! Extracts text regions from image spans by delegating to the
//! nvisy-ocr [`OcrEngine`]'s text recognition pipeline.
//!
//! [`OcrEngine`]: nvisy_ocr::OcrEngine

use nvisy_codec::document::Span;
use nvisy_codec::handler::ImageData;
use nvisy_core::Error;
use nvisy_ocr::{ImageFormat, ImageInput, ImageOutput, OcrEngine, RunParams};

use crate::operation::{Operation, ParallelContext};

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

    fn to_image_input(span: &Span<(), ImageData>) -> Result<ImageInput, Error> {
        let png_bytes = span.data.encode_png()?;
        Ok(ImageInput::with_source(
            span.source,
            png_bytes,
            ImageFormat::Png,
        ))
    }
}

impl Operation for Ocr {
    type Input = ParallelContext<Vec<Span<(), ImageData>>>;
    type Output = ParallelContext<Vec<ImageOutput>>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output, Error> {
        let shared = input.shared.clone();
        let spans = input.into_inner();

        if spans.is_empty() {
            return Ok(ParallelContext::new(Vec::new(), shared));
        }

        let images = spans
            .iter()
            .map(Self::to_image_input)
            .collect::<Result<Vec<_>, _>>()?;

        let outputs = self.engine.run_batch(&images, &self.params).await?;

        Ok(ParallelContext::new(outputs, shared))
    }
}

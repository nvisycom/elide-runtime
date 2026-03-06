//! OCR text extraction from images.
//!
//! Extracts text regions from image spans by delegating to the
//! nvisy-ocr [`Engine`]'s text recognition pipeline.
//!
//! [`Engine`]: nvisy_ocr::Engine

use nvisy_codec::document::Span;
use nvisy_codec::handler::ImageData;
use nvisy_core::Error;
use nvisy_ocr::{Engine, ImageFormat, ImageInput, ImageOutput, RunParams};

use crate::operation::{Operation, ParallelContext};

/// OCR text-extraction operation: thin adapter around [`Engine`].
///
/// [`Engine`]: nvisy_ocr::Engine
pub struct Ocr {
    engine: Engine,
    params: RunParams,
}

impl Ocr {
    /// Create a new OCR operation from a pre-built engine.
    pub fn new(engine: Engine, params: RunParams) -> Self {
        Self { engine, params }
    }

    fn to_image_input(span: &Span<(), ImageData>) -> Result<ImageInput, Error> {
        let png_bytes = span.data.encode_png()?;
        Ok(ImageInput::with_source(
            span.source.clone(),
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

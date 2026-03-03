//! Optical character recognition operation.

use crate::operation::Operation;
use nvisy_core::Error;

/// Extracts text from image content via OCR.
pub struct Ocr;

impl Operation for Ocr {
    type Input = ();
    type Output = ();
    type Context = ();

    async fn call(&self, _input: Self::Input, _ctx: Self::Context) -> Result<Self::Output, Error> {
        todo!("OCR operation not yet implemented")
    }
}

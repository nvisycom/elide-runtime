//! Named entity recognition operation.

use crate::operation::Operation;
use nvisy_core::Error;

/// Detects named entities (PII, PHI, etc.) in text content.
pub struct Ner;

impl Operation for Ner {
    type Input = ();
    type Output = ();
    type Context = ();

    async fn call(&self, _input: Self::Input, _ctx: Self::Context) -> Result<Self::Output, Error> {
        todo!("NER operation not yet implemented")
    }
}

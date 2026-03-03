//! Content ingestion operation.

use crate::operation::Operation;
use crate::operation::ParallelContext;
use nvisy_core::Error;

/// Ingestions raw content into the pipeline.
pub struct Ingestion;

impl Operation for Ingestion {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Ingestion operation not yet implemented")
    }
}

//! Content ingestion operation.

use nvisy_core::Error;

use crate::operation::{Operation, ParallelContext};

/// Ingestions raw content into the pipeline.
pub struct Ingestion;

impl Operation for Ingestion {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Ingestion operation not yet implemented")
    }
}

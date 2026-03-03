//! Content ingestion operation.

use crate::operation::Operation;
use crate::operation::ParallelContext;
use nvisy_core::Error;

/// Ingests raw content into the pipeline.
pub struct Ingest;

impl Operation for Ingest {
    type Input = ();
    type Output = ();
    type Context = ParallelContext;

    async fn call(&self, _input: Self::Input, _ctx: Self::Context) -> Result<Self::Output, Error> {
        todo!("Ingest operation not yet implemented")
    }
}

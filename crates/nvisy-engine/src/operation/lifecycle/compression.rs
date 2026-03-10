//! Content compression operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

#[allow(dead_code)]
const TARGET: &str = "nvisy_engine::op::compression";

/// Compresses content for storage or transfer.
pub struct Compression;

impl Operation for Compression {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output> {
        todo!("Compression operation not yet implemented")
    }
}

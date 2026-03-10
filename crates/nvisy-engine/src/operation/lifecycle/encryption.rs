//! Content encryption operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

#[allow(dead_code)]
const TARGET: &str = "nvisy_engine::op::encryption";

/// Encrypts content at rest or in transit.
pub struct Encryption;

impl Operation for Encryption {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output> {
        todo!("Encryption operation not yet implemented")
    }
}

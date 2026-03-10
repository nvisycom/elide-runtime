//! Content encryption operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::encryption";

/// Encrypts content at rest or in transit.
pub struct Encryption;

impl Encryption {
    async fn encrypt(&self, _data: ()) -> Result<()> {
        tracing::debug!(target: TARGET, "encrypting content");
        todo!("Encryption operation not yet implemented")
    }
}

impl Operation for Encryption {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.encrypt(data)).await
    }
}

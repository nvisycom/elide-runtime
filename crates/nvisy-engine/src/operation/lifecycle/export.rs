//! Content export operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::export";

/// Exports redacted content to a downstream target.
pub struct Export;

impl Export {
    async fn export(&self, _data: ()) -> Result<()> {
        tracing::debug!(target: TARGET, "exporting content");
        todo!("Export operation not yet implemented")
    }
}

impl Operation for Export {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.export(data)).await
    }
}

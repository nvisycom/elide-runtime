//! The `Action` trait -- the fundamental processing unit in a pipeline.

use serde::de::DeserializeOwned;
use tokio::sync::mpsc;

use crate::datatypes::blob::Blob;
use crate::error::Error;

/// A processing step that consumes blobs from an input channel and
/// produces blobs to an output channel.
///
/// Actions are the primary unit of work in a pipeline. Each action
/// receives blobs via an async MPSC channel, transforms them (possibly
/// attaching artifacts), and forwards results to the next stage.
///
/// Actions that need a provider client should hold it as a struct field
/// rather than receiving it as a parameter.
#[async_trait::async_trait]
pub trait Action: Send + Sync + 'static {
    /// Strongly-typed parameters for this action.
    type Params: DeserializeOwned + Send;

    /// Unique identifier for this action (e.g. "detect-regex").
    fn id(&self) -> &str;

    /// Validate action parameters.
    fn validate_params(&self, params: &Self::Params) -> Result<(), Error>;

    /// Execute the action, consuming blobs from input and sending results to output.
    /// Returns the number of items processed.
    async fn execute(
        &self,
        input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        params: Self::Params,
    ) -> Result<u64, Error>;
}

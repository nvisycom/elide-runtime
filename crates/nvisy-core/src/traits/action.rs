//! The `Action` trait -- the fundamental processing unit in a pipeline.

use std::any::Any;

use tokio::sync::mpsc;

use crate::datatypes::blob::Blob;
use crate::error::Error;

/// A processing step that consumes blobs from an input channel and
/// produces blobs to an output channel.
///
/// Actions are the primary unit of work in a pipeline. Each action
/// receives blobs via an async MPSC channel, transforms them (possibly
/// attaching artifacts), and forwards results to the next stage.
#[async_trait::async_trait]
pub trait Action: Send + Sync + 'static {
    /// Unique identifier for this action (e.g. "detect-regex").
    fn id(&self) -> &str;

    /// Whether this action requires a provider client.
    fn requires_client(&self) -> bool {
        false
    }

    /// The provider ID this action requires, if any.
    fn required_provider_id(&self) -> Option<&str> {
        None
    }

    /// Validate action parameters.
    fn validate_params(&self, params: &serde_json::Value) -> Result<(), Error>;

    /// Execute the action, consuming blobs from input and sending results to output.
    /// Returns the number of items processed.
    async fn execute(
        &self,
        input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        params: serde_json::Value,
        client: Option<Box<dyn Any + Send>>,
    ) -> Result<u64, Error>;
}

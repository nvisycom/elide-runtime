use std::any::Any;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::data::DataValue;
use crate::errors::NvisyError;

/// Type-erased action that consumes from an input channel and produces to an output channel.
#[async_trait]
pub trait Action: Send + Sync + 'static {
    /// Unique identifier for this action (e.g. "detect-regex").
    fn id(&self) -> &str;

    /// Expected input data type name (e.g. "document").
    fn input_type(&self) -> &str;

    /// Output data type name (e.g. "entity").
    fn output_type(&self) -> &str;

    /// Whether this action requires a provider client.
    fn requires_client(&self) -> bool {
        false
    }

    /// The provider ID this action requires, if any.
    fn required_provider_id(&self) -> Option<&str> {
        None
    }

    /// Validate action parameters.
    fn validate_params(&self, params: &serde_json::Value) -> Result<(), NvisyError>;

    /// Execute the action, consuming items from input and sending results to output.
    /// Returns the number of items processed.
    async fn execute(
        &self,
        input: mpsc::Receiver<DataValue>,
        output: mpsc::Sender<DataValue>,
        params: serde_json::Value,
        client: Option<Box<dyn Any + Send>>,
    ) -> Result<u64, NvisyError>;
}

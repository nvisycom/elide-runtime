use std::any::Any;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::data::DataValue;
use crate::errors::NvisyError;

/// A source stream that reads data from an external system into the pipeline.
#[async_trait]
pub trait StreamSource: Send + Sync + 'static {
    fn id(&self) -> &str;
    fn output_type(&self) -> &str;
    fn required_provider_id(&self) -> &str;
    fn validate_params(&self, params: &serde_json::Value) -> Result<(), NvisyError>;

    async fn read(
        &self,
        output: mpsc::Sender<DataValue>,
        params: serde_json::Value,
        client: Box<dyn Any + Send>,
    ) -> Result<u64, NvisyError>;
}

/// A target stream that writes pipeline data to an external system.
#[async_trait]
pub trait StreamTarget: Send + Sync + 'static {
    fn id(&self) -> &str;
    fn input_type(&self) -> &str;
    fn required_provider_id(&self) -> &str;
    fn validate_params(&self, params: &serde_json::Value) -> Result<(), NvisyError>;

    async fn write(
        &self,
        input: mpsc::Receiver<DataValue>,
        params: serde_json::Value,
        client: Box<dyn Any + Send>,
    ) -> Result<u64, NvisyError>;
}

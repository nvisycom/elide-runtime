//! Stream source and target traits for external I/O.

use std::any::Any;

use tokio::sync::mpsc;

use crate::datatypes::blob::Blob;
use crate::error::Error;

/// A source stream that reads blobs from an external system into the pipeline.
///
/// Implementations connect to a storage backend (e.g. S3, local filesystem)
/// and emit blobs into the pipeline's input channel.
#[async_trait::async_trait]
pub trait StreamSource: Send + Sync + 'static {
    /// Unique identifier for this stream source (e.g. `"s3-read"`).
    fn id(&self) -> &str;
    /// The provider this stream requires (e.g. `"s3"`).
    fn required_provider_id(&self) -> &str;
    /// Validate source parameters before execution.
    fn validate_params(&self, params: &serde_json::Value) -> Result<(), Error>;

    /// Read blobs from the external system and send them to `output`.
    ///
    /// Returns the number of blobs read.
    async fn read(
        &self,
        output: mpsc::Sender<Blob>,
        params: serde_json::Value,
        client: Box<dyn Any + Send>,
    ) -> Result<u64, Error>;
}

/// A target stream that writes blobs from the pipeline to an external system.
///
/// Implementations receive processed blobs from the pipeline and persist
/// them to a storage backend.
#[async_trait::async_trait]
pub trait StreamTarget: Send + Sync + 'static {
    /// Unique identifier for this stream target (e.g. `"s3-write"`).
    fn id(&self) -> &str;
    /// The provider this stream requires (e.g. `"s3"`).
    fn required_provider_id(&self) -> &str;
    /// Validate target parameters before execution.
    fn validate_params(&self, params: &serde_json::Value) -> Result<(), Error>;

    /// Receive blobs from `input` and write them to the external system.
    ///
    /// Returns the number of blobs written.
    async fn write(
        &self,
        input: mpsc::Receiver<Blob>,
        params: serde_json::Value,
        client: Box<dyn Any + Send>,
    ) -> Result<u64, Error>;
}

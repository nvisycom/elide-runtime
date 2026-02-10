//! Stream source and target traits for external I/O.

use serde::de::DeserializeOwned;
use tokio::sync::mpsc;

use crate::datatypes::blob::Blob;
use crate::error::Error;

/// A source stream that reads blobs from an external system into the pipeline.
///
/// Implementations connect to a storage backend (e.g. S3, local filesystem)
/// and emit blobs into the pipeline's input channel.
#[async_trait::async_trait]
pub trait StreamSource: Send + Sync + 'static {
    /// Strongly-typed parameters for this stream source.
    type Params: DeserializeOwned + Send;
    /// The client type this stream requires.
    type Client: Send + 'static;

    /// Unique identifier for this stream source (e.g. `"s3-read"`).
    fn id(&self) -> &str;
    /// Validate source parameters before execution.
    fn validate_params(&self, params: &Self::Params) -> Result<(), Error>;

    /// Read blobs from the external system and send them to `output`.
    ///
    /// Returns the number of blobs read.
    async fn read(
        &self,
        output: mpsc::Sender<Blob>,
        params: Self::Params,
        client: Self::Client,
    ) -> Result<u64, Error>;
}

/// A target stream that writes blobs from the pipeline to an external system.
///
/// Implementations receive processed blobs from the pipeline and persist
/// them to a storage backend.
#[async_trait::async_trait]
pub trait StreamTarget: Send + Sync + 'static {
    /// Strongly-typed parameters for this stream target.
    type Params: DeserializeOwned + Send;
    /// The client type this stream requires.
    type Client: Send + 'static;

    /// Unique identifier for this stream target (e.g. `"s3-write"`).
    fn id(&self) -> &str;
    /// Validate target parameters before execution.
    fn validate_params(&self, params: &Self::Params) -> Result<(), Error>;

    /// Receive blobs from `input` and write them to the external system.
    ///
    /// Returns the number of blobs written.
    async fn write(
        &self,
        input: mpsc::Receiver<Blob>,
        params: Self::Params,
        client: Self::Client,
    ) -> Result<u64, Error>;
}

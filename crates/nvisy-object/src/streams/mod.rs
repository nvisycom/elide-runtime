//! Streaming read and write adapters for object stores.

use serde::de::DeserializeOwned;
use tokio::sync::mpsc;

use nvisy_core::io::ContentData;
use nvisy_core::error::Error;

/// A source stream that reads content from an external system into the pipeline.
///
/// Implementations connect to a storage backend (e.g. S3, local filesystem)
/// and emit content data into the pipeline's input channel.
#[async_trait::async_trait]
pub trait StreamSource: Send + Sync + 'static {
    /// Strongly-typed parameters for this stream source.
    type Params: DeserializeOwned + Send;
    /// The client type this stream requires.
    type Client: Send + 'static;

    /// Unique identifier for this stream source (e.g. `"s3-read"`).
    fn id(&self) -> &str;

    /// Read content from the external system and send it to `output`.
    ///
    /// Returns the number of items read.
    async fn read(
        &self,
        output: mpsc::Sender<ContentData>,
        params: Self::Params,
        client: Self::Client,
    ) -> Result<u64, Error>;
}

/// A target stream that writes content from the pipeline to an external system.
///
/// Implementations receive processed content data from the pipeline and persist
/// it to a storage backend.
#[async_trait::async_trait]
pub trait StreamTarget: Send + Sync + 'static {
    /// Strongly-typed parameters for this stream target.
    type Params: DeserializeOwned + Send;
    /// The client type this stream requires.
    type Client: Send + 'static;

    /// Unique identifier for this stream target (e.g. `"s3-write"`).
    fn id(&self) -> &str;

    /// Receive content from `input` and write it to the external system.
    ///
    /// Returns the number of items written.
    async fn write(
        &self,
        input: mpsc::Receiver<ContentData>,
        params: Self::Params,
        client: Self::Client,
    ) -> Result<u64, Error>;
}

mod read;
mod write;

pub use read::{ObjectReadStream, ObjectReadParams};
pub use write::{ObjectWriteStream, ObjectWriteParams};

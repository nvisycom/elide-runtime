//! Streaming source trait for pipeline input.
//!
//! [`StreamSource`] reads content from an external system into the pipeline.

use serde::de::DeserializeOwned;
use tokio::sync::mpsc;

use nvisy_core::Error;
use nvisy_core::io::ContentData;

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

    /// Unique identifier for this stream source (e.g. `"read"`).
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

//! Streaming target trait for pipeline output.
//!
//! [`StreamTarget`] writes processed content back to an external system.

use serde::de::DeserializeOwned;
use tokio::sync::mpsc;

use nvisy_core::Error;
use nvisy_core::io::ContentData;

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

    /// Unique identifier for this stream target (e.g. `"write"`).
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

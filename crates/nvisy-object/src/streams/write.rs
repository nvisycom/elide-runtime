//! Streaming writer that uploads blobs to an S3-compatible store.

use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::error::Error;
use super::StreamTarget;
use crate::client::ObjectStoreBox;

/// Typed parameters for [`ObjectWriteStream`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectWriteParams {
    /// Key prefix prepended to each blob path.
    #[serde(default)]
    pub prefix: String,
}

/// A [`StreamTarget`] that receives [`Blob`]s from the input channel and
/// uploads each one to an S3-compatible object store.
pub struct ObjectWriteStream;

#[async_trait::async_trait]
impl StreamTarget for ObjectWriteStream {
    type Params = ObjectWriteParams;
    type Client = ObjectStoreBox;

    fn id(&self) -> &str { "write" }

    fn validate_params(&self, _params: &Self::Params) -> Result<(), Error> {
        Ok(())
    }

    async fn write(
        &self,
        mut input: mpsc::Receiver<Blob>,
        params: Self::Params,
        client: Self::Client,
    ) -> Result<u64, Error> {
        let store_client = &client.0;

        let prefix = &params.prefix;
        let mut total = 0u64;

        while let Some(blob) = input.recv().await {
            let key = if prefix.is_empty() {
                blob.path.clone()
            } else {
                format!("{}{}", prefix, blob.path)
            };

            store_client
                .put(&key, blob.content.clone(), blob.content_type())
                .await
                .map_err(|e| Error::runtime(format!("Put failed for {}: {}", key, e), "object/write", true))?;

            total += 1;
        }

        Ok(total)
    }
}

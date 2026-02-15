//! Streaming writer that uploads content to an S3-compatible store.

use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::io::ContentData;
use nvisy_core::error::Error;
use super::StreamTarget;
use crate::client::ObjectStoreBox;

/// Typed parameters for [`ObjectWriteStream`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectWriteParams {
    /// Key prefix prepended to each content source UUID.
    #[serde(default)]
    pub prefix: String,
}

/// A [`StreamTarget`] that receives [`ContentData`] from the input channel and
/// uploads each one to an S3-compatible object store.
pub struct ObjectWriteStream;

#[async_trait::async_trait]
impl StreamTarget for ObjectWriteStream {
    type Params = ObjectWriteParams;
    type Client = ObjectStoreBox;

    fn id(&self) -> &str { "write" }

    async fn write(
        &self,
        mut input: mpsc::Receiver<ContentData>,
        params: Self::Params,
        client: Self::Client,
    ) -> Result<u64, Error> {
        let store_client = &client.0;

        let prefix = &params.prefix;
        let mut total = 0u64;

        while let Some(content) = input.recv().await {
            let source_id = content.content_source.to_string();
            let key = if prefix.is_empty() {
                source_id
            } else {
                format!("{}{}", prefix, source_id)
            };

            store_client
                .put(&key, content.to_bytes(), content.content_type())
                .await
                .map_err(|e| Error::runtime(format!("Put failed for {}: {}", key, e), "object/write", true))?;

            total += 1;
        }

        Ok(total)
    }
}

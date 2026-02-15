//! Streaming reader that pulls objects from an S3-compatible store.

use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;
use nvisy_core::error::Error;
use super::StreamSource;
use crate::client::ObjectStoreBox;

/// Typed parameters for [`ObjectReadStream`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectReadParams {
    /// Object key prefix to filter by.
    #[serde(default)]
    pub prefix: String,
    /// Number of keys to fetch per page.
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

fn default_batch_size() -> usize { 100 }

/// A [`StreamSource`] that lists and fetches objects from an S3-compatible store,
/// emitting each object as a [`ContentData`] onto the output channel.
pub struct ObjectReadStream;

#[async_trait::async_trait]
impl StreamSource for ObjectReadStream {
    type Params = ObjectReadParams;
    type Client = ObjectStoreBox;

    fn id(&self) -> &str { "read" }

    async fn read(
        &self,
        output: mpsc::Sender<ContentData>,
        params: Self::Params,
        client: Self::Client,
    ) -> Result<u64, Error> {
        let store_client = &client.0;

        let prefix = &params.prefix;
        let batch_size = params.batch_size;

        let mut cursor: Option<String> = None;
        let mut total = 0u64;

        loop {
            let result = store_client
                .list(prefix, cursor.as_deref())
                .await
                .map_err(|e| Error::runtime(format!("List failed: {}", e), "object/read", true))?;

            let keys_count = result.keys.len();

            for key in &result.keys {
                let get_result = store_client
                    .get(key)
                    .await
                    .map_err(|e| Error::runtime(format!("Get failed for {}: {}", key, e), "object/read", true))?;

                let mut content = ContentData::new(ContentSource::new(), get_result.data);
                if let Some(ct) = get_result.content_type {
                    content = content.with_content_type(ct);
                }

                total += 1;
                if output.send(content).await.is_err() {
                    return Ok(total);
                }
            }

            if keys_count < batch_size || result.next_cursor.is_none() {
                break;
            }
            cursor = result.next_cursor;
        }

        Ok(total)
    }
}

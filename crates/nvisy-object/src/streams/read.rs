//! Streaming reader that pulls objects from an S3-compatible store.

use std::any::Any;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::error::Error;
use nvisy_core::traits::stream::StreamSource;
use crate::client::ObjectStoreBox;

/// A [`StreamSource`] that lists and fetches objects from an S3-compatible store,
/// emitting each object as a [`Blob`] onto the output channel.
///
/// # Parameters (JSON)
///
/// - `prefix` -- object key prefix to filter by (default: `""`).
/// - `batchSize` -- number of keys to fetch per page (default: `100`).
pub struct ObjectReadStream;

#[async_trait::async_trait]
impl StreamSource for ObjectReadStream {
    fn id(&self) -> &str { "read" }
    fn required_provider_id(&self) -> &str { "s3" }

    fn validate_params(&self, _params: &serde_json::Value) -> Result<(), Error> {
        Ok(())
    }

    async fn read(
        &self,
        output: mpsc::Sender<Blob>,
        params: serde_json::Value,
        client: Box<dyn Any + Send>,
    ) -> Result<u64, Error> {
        let store_box = client.downcast::<ObjectStoreBox>().map_err(|_| {
            Error::runtime("Invalid client type for object read stream", "object/read", false)
        })?;
        let store_client = &store_box.0;

        let prefix = params.get("prefix").and_then(|v| v.as_str()).unwrap_or("");
        let batch_size = params.get("batchSize").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

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

                let mut blob = Blob::new(key.clone(), get_result.data);
                if let Some(ct) = get_result.content_type {
                    blob = blob.with_content_type(ct);
                }

                total += 1;
                if output.send(blob).await.is_err() {
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

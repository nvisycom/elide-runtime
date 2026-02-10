//! Streaming writer that uploads blobs to an S3-compatible store.

use std::any::Any;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::error::Error;
use nvisy_core::traits::stream::StreamTarget;
use crate::client::ObjectStoreBox;

/// A [`StreamTarget`] that receives [`Blob`]s from the input channel and
/// uploads each one to an S3-compatible object store.
///
/// # Parameters (JSON)
///
/// - `prefix` -- key prefix prepended to each blob path (default: `""`).
pub struct ObjectWriteStream;

#[async_trait::async_trait]
impl StreamTarget for ObjectWriteStream {
    fn id(&self) -> &str { "write" }
    fn required_provider_id(&self) -> &str { "s3" }

    fn validate_params(&self, _params: &serde_json::Value) -> Result<(), Error> {
        Ok(())
    }

    async fn write(
        &self,
        mut input: mpsc::Receiver<Blob>,
        params: serde_json::Value,
        client: Box<dyn Any + Send>,
    ) -> Result<u64, Error> {
        let store_box = client.downcast::<ObjectStoreBox>().map_err(|_| {
            Error::runtime("Invalid client type for object write stream", "object/write", false)
        })?;
        let store_client = &store_box.0;

        let prefix = params.get("prefix").and_then(|v| v.as_str()).unwrap_or("");
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

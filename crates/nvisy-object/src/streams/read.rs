use std::any::Any;
use async_trait::async_trait;
use tokio::sync::mpsc;

use nvisy_core::data::DataValue;
use nvisy_core::datatypes::blob::Blob;
use nvisy_core::errors::NvisyError;
use nvisy_core::traits::stream::StreamSource;
use crate::client::ObjectStoreBox;

pub struct ObjectReadStream;

#[async_trait]
impl StreamSource for ObjectReadStream {
    fn id(&self) -> &str { "read" }
    fn output_type(&self) -> &str { "blob" }
    fn required_provider_id(&self) -> &str { "s3" }

    fn validate_params(&self, _params: &serde_json::Value) -> Result<(), NvisyError> {
        Ok(())
    }

    async fn read(
        &self,
        output: mpsc::Sender<DataValue>,
        params: serde_json::Value,
        client: Box<dyn Any + Send>,
    ) -> Result<u64, NvisyError> {
        let store_box = client.downcast::<ObjectStoreBox>().map_err(|_| {
            NvisyError::runtime("Invalid client type for object read stream", "object/read", false)
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
                .map_err(|e| NvisyError::runtime(format!("List failed: {}", e), "object/read", true))?;

            let keys_count = result.keys.len();

            for key in &result.keys {
                let get_result = store_client
                    .get(key)
                    .await
                    .map_err(|e| NvisyError::runtime(format!("Get failed for {}: {}", key, e), "object/read", true))?;

                let mut blob = Blob::new(key.clone(), get_result.data);
                if let Some(ct) = get_result.content_type {
                    blob = blob.with_content_type(ct);
                }

                total += 1;
                if output.send(DataValue::Blob(blob)).await.is_err() {
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

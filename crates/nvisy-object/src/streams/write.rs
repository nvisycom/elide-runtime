use std::any::Any;
use async_trait::async_trait;
use tokio::sync::mpsc;

use nvisy_core::data::DataValue;
use nvisy_core::errors::NvisyError;
use nvisy_core::traits::stream::StreamTarget;
use crate::client::ObjectStoreBox;

pub struct ObjectWriteStream;

#[async_trait]
impl StreamTarget for ObjectWriteStream {
    fn id(&self) -> &str { "write" }
    fn input_type(&self) -> &str { "blob" }
    fn required_provider_id(&self) -> &str { "s3" }

    fn validate_params(&self, _params: &serde_json::Value) -> Result<(), NvisyError> {
        Ok(())
    }

    async fn write(
        &self,
        mut input: mpsc::Receiver<DataValue>,
        params: serde_json::Value,
        client: Box<dyn Any + Send>,
    ) -> Result<u64, NvisyError> {
        let store_box = client.downcast::<ObjectStoreBox>().map_err(|_| {
            NvisyError::runtime("Invalid client type for object write stream", "object/write", false)
        })?;
        let store_client = &store_box.0;

        let prefix = params.get("prefix").and_then(|v| v.as_str()).unwrap_or("");
        let mut total = 0u64;

        while let Some(item) = input.recv().await {
            if let DataValue::Blob(blob) = item {
                let key = if prefix.is_empty() {
                    blob.path.clone()
                } else {
                    format!("{}{}", prefix, blob.path)
                };

                store_client
                    .put(&key, blob.content.clone(), blob.content_type())
                    .await
                    .map_err(|e| NvisyError::runtime(format!("Put failed for {}: {}", key, e), "object/write", true))?;

                total += 1;
            }
        }

        Ok(total)
    }
}

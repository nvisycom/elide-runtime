//! Streaming writer that uploads content to a cloud object store.

use object_store::PutMode;
use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::Error;
use nvisy_core::io::ContentData;

use nvisy_pipeline::stream::StreamTarget;

use crate::client::ObjectStoreClient;

/// Typed parameters for [`ObjectWriteStream`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectWriteParams {
    /// Key prefix prepended to each content source UUID.
    #[serde(default)]
    pub prefix: String,
    /// When `true`, uses `PutMode::Create` so that writing to an existing
    /// key fails with an error.
    #[serde(default)]
    pub create_only: bool,
}

/// A [`StreamTarget`] that receives [`ContentData`] from the input channel and
/// uploads each one to a cloud object store.
pub struct ObjectWriteStream;

#[async_trait::async_trait]
impl StreamTarget for ObjectWriteStream {
    type Params = ObjectWriteParams;
    type Client = ObjectStoreClient;

    fn id(&self) -> &str { "write" }

    #[tracing::instrument(name = "object.write", skip_all, fields(prefix = %params.prefix, count))]
    async fn write(
        &self,
        mut input: mpsc::Receiver<ContentData>,
        params: Self::Params,
        client: Self::Client,
    ) -> Result<u64, Error> {
        let prefix = &params.prefix;
        let mut total = 0u64;

        while let Some(content) = input.recv().await {
            let source_id = content.content_source.to_string();
            let key = if prefix.is_empty() {
                source_id
            } else {
                format!("{prefix}{source_id}")
            };

            if params.create_only {
                client
                    .put_opts(&key, content.to_bytes(), PutMode::Create, content.content_type())
                    .await?;
            } else {
                client
                    .put(&key, content.to_bytes(), content.content_type())
                    .await?;
            }

            total += 1;
        }

        tracing::Span::current().record("count", total);
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use nvisy_core::io::ContentData;
    use nvisy_core::path::ContentSource;
    use object_store::memory::InMemory;

    fn test_client() -> ObjectStoreClient {
        ObjectStoreClient::new(InMemory::new())
    }

    #[tokio::test]
    async fn write_uploads_all() {
        let client = test_client();
        let (tx, rx) = mpsc::channel(16);

        let sources: Vec<ContentSource> = (0..3).map(|_| ContentSource::new()).collect();
        for (i, src) in sources.iter().enumerate() {
            let content = ContentData::new(*src, Bytes::from(format!("payload-{i}")));
            tx.send(content).await.unwrap();
        }
        drop(tx);

        let stream = ObjectWriteStream;
        let params = ObjectWriteParams {
            prefix: "out/".to_string(),
            create_only: false,
        };

        let count = stream.write(rx, params, client.clone()).await.unwrap();
        assert_eq!(count, 3);

        // Verify all objects were stored
        let items = client.list("out/").await.unwrap();
        assert_eq!(items.len(), 3);
    }

    #[tokio::test]
    async fn write_create_only() {
        let client = test_client();

        // Pre-populate an object at a known key
        let source = ContentSource::new();
        let key = format!("prefix/{source}");
        client
            .put(&key, Bytes::from("existing"), None)
            .await
            .unwrap();

        // Try to write the same key with create_only
        let (tx, rx) = mpsc::channel(1);
        let content = ContentData::new(source, Bytes::from("new"));
        tx.send(content).await.unwrap();
        drop(tx);

        let stream = ObjectWriteStream;
        let params = ObjectWriteParams {
            prefix: "prefix/".to_string(),
            create_only: true,
        };

        let result = stream.write(rx, params, client).await;
        assert!(result.is_err());
    }
}

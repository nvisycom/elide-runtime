//! Streaming reader that pulls objects from a cloud object store.

use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::Error;
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;

use nvisy_pipeline::stream::StreamSource;

use crate::client::ObjectStoreClient;

/// Typed parameters for [`ObjectReadStream`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectReadParams {
    /// Object key prefix to filter by.
    #[serde(default)]
    pub prefix: String,
    /// Skip objects whose size exceeds this limit (in bytes).
    #[serde(default)]
    pub max_size: Option<u64>,
}

/// A [`StreamSource`] that lists and fetches objects from a cloud object store,
/// emitting each object as a [`ContentData`] onto the output channel.
pub struct ObjectReadStream;

#[async_trait::async_trait]
impl StreamSource for ObjectReadStream {
    type Params = ObjectReadParams;
    type Client = ObjectStoreClient;

    fn id(&self) -> &str { "read" }

    #[tracing::instrument(name = "object.read", skip_all, fields(prefix = %params.prefix, count))]
    async fn read(
        &self,
        output: mpsc::Sender<ContentData>,
        params: Self::Params,
        client: Self::Client,
    ) -> Result<u64, Error> {
        let mut stream = client.list_stream(&params.prefix);
        let mut total = 0u64;

        while let Some(result) = stream.next().await {
            let meta = result?;
            let key = meta.location.as_ref();

            if let Some(max) = params.max_size {
                if meta.size > max {
                    tracing::debug!(key, size = meta.size, max_size = max, "skipping oversized object");
                    continue;
                }
            }

            let source = ContentSource::new();
            tracing::debug!(key, source_id = %source, "fetching object");

            let result = client.get(key).await?;

            let mut content = ContentData::new(source, result.data);
            if let Some(ct) = result.content_type {
                content = content.with_content_type(ct);
            }

            total += 1;
            if output.send(content).await.is_err() {
                break;
            }
        }

        tracing::Span::current().record("count", total);
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use object_store::memory::InMemory;

    fn test_client() -> ObjectStoreClient {
        ObjectStoreClient::new(InMemory::new())
    }

    #[tokio::test]
    async fn read_emits_all_objects() {
        let client = test_client();
        for i in 0..3 {
            client
                .put(
                    &format!("data/file{i}.txt"),
                    Bytes::from(format!("content-{i}")),
                    Some("text/plain"),
                )
                .await
                .unwrap();
        }

        let (tx, mut rx) = mpsc::channel(16);
        let stream = ObjectReadStream;
        let params = ObjectReadParams {
            prefix: "data/".to_string(),
            max_size: None,
        };

        let count = stream.read(tx, params, client).await.unwrap();
        assert_eq!(count, 3);

        let mut items = Vec::new();
        while let Some(item) = rx.recv().await {
            items.push(item);
        }
        assert_eq!(items.len(), 3);
    }

    #[tokio::test]
    async fn read_max_size_filter() {
        let client = test_client();
        client
            .put("small.bin", Bytes::from("hi"), None)
            .await
            .unwrap();
        client
            .put("big.bin", Bytes::from("this is a much bigger payload"), None)
            .await
            .unwrap();

        let (tx, mut rx) = mpsc::channel(16);
        let stream = ObjectReadStream;
        let params = ObjectReadParams {
            prefix: String::new(),
            max_size: Some(10),
        };

        let count = stream.read(tx, params, client).await.unwrap();
        assert_eq!(count, 1);

        let item = rx.recv().await.unwrap();
        assert_eq!(item.as_bytes(), b"hi");
        assert!(rx.recv().await.is_none());
    }
}

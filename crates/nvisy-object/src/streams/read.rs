//! Streaming reader that pulls objects from a cloud object store.

use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::error::Error;
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
        let objects = client
            .list(&params.prefix)
            .await
            .map_err(Error::from)?;

        let mut total = 0u64;

        for meta in &objects {
            let key = meta.location.as_ref();
            let result = client.get(key).await.map_err(Error::from)?;

            let mut content = ContentData::new(ContentSource::new(), result.data);
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

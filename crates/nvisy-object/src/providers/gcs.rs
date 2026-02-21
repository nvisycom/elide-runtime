//! Google Cloud Storage provider using [`object_store::gcp::GoogleCloudStorageBuilder`].

use object_store::gcp::GoogleCloudStorageBuilder;
use serde::Deserialize;

use nvisy_core::Error;
use nvisy_pipeline::provider::Provider;

use crate::client::ObjectStoreClient;

/// Typed credentials for Google Cloud Storage.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GcsCredentials {
    /// GCS bucket name.
    pub bucket: String,
    /// Path to a JSON service account key file.
    #[serde(default)]
    pub service_account_key: Option<String>,
    /// Custom endpoint URL (for testing with a fake GCS server).
    #[serde(default)]
    pub endpoint: Option<String>,
}

/// Factory that creates [`ObjectStoreClient`] instances backed by Google Cloud Storage.
pub struct GcsProvider;

#[async_trait::async_trait]
impl Provider for GcsProvider {
    type Credentials = GcsCredentials;
    type Client = ObjectStoreClient;

    const ID: &str = "gcs";

    async fn verify(creds: &Self::Credentials) -> Result<(), Error> {
        let client = Self::connect(creds).await?;
        client.verify_reachable().await
    }

    async fn connect(creds: &Self::Credentials) -> Result<Self::Client, Error> {
        let mut builder =
            GoogleCloudStorageBuilder::new().with_bucket_name(&creds.bucket);

        if let Some(key_path) = &creds.service_account_key {
            builder = builder.with_service_account_key(key_path);
        }

        if let Some(endpoint) = &creds.endpoint {
            builder = builder.with_url(endpoint);
        }

        let store = builder
            .build()
            .map_err(|e| Error::connection(e.to_string(), "gcs", true))?;

        Ok(ObjectStoreClient::new(store))
    }
}

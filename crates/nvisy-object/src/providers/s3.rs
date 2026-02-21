//! S3-compatible provider using [`object_store::aws::AmazonS3Builder`].
//!
//! Works with AWS S3, MinIO, and any S3-compatible service.

use object_store::aws::AmazonS3Builder;
use serde::Deserialize;

use nvisy_core::Error;
use nvisy_pipeline::provider::Provider;

use crate::client::ObjectStoreClient;

/// Typed credentials for S3-compatible provider.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct S3Credentials {
    /// S3 bucket name.
    pub bucket: String,
    /// AWS region (defaults to `us-east-1`).
    #[serde(default = "default_region")]
    pub region: String,
    /// Endpoint URL (e.g. `http://localhost:9000` for MinIO).
    /// Required for non-AWS S3-compatible services.
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Access key ID for static credentials.
    #[serde(default)]
    pub access_key_id: Option<String>,
    /// Secret access key for static credentials.
    #[serde(default)]
    pub secret_access_key: Option<String>,
    /// Session token for temporary credentials.
    #[serde(default)]
    pub session_token: Option<String>,
}

fn default_region() -> String {
    "us-east-1".to_string()
}

/// Factory that creates [`ObjectStoreClient`] instances backed by S3.
pub struct S3Provider;

#[async_trait::async_trait]
impl Provider for S3Provider {
    type Credentials = S3Credentials;
    type Client = ObjectStoreClient;

    const ID: &str = "s3";

    async fn verify(creds: &Self::Credentials) -> Result<(), Error> {
        let client = Self::connect(creds).await?;
        client.verify_reachable().await
    }

    async fn connect(creds: &Self::Credentials) -> Result<Self::Client, Error> {
        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(&creds.bucket)
            .with_region(&creds.region);

        if let Some(endpoint) = &creds.endpoint {
            builder = builder.with_endpoint(endpoint);
            if endpoint.starts_with("http://") {
                builder = builder.with_allow_http(true);
            }
        }

        if let Some(access_key) = &creds.access_key_id {
            builder = builder.with_access_key_id(access_key);
        }

        if let Some(secret_key) = &creds.secret_access_key {
            builder = builder.with_secret_access_key(secret_key);
        }

        if let Some(token) = &creds.session_token {
            builder = builder.with_token(token);
        }

        let store = builder
            .build()
            .map_err(|e| Error::connection(e.to_string(), "s3", true))?;

        Ok(ObjectStoreClient::new(store))
    }
}

//! S3-compatible provider implementation using the MinIO Rust SDK.
//!
//! Provides [`S3ObjectStoreClient`] which implements [`ObjectStoreClient`] and
//! [`S3Provider`] which plugs into the engine's provider system.
//!
//! Works with MinIO, AWS S3, and any S3-compatible service.

use bytes::Bytes;
use serde::Deserialize;

use minio::s3::creds::StaticProvider;
use minio::s3::http::BaseUrl;
use minio::s3::types::{S3Api, ToStream};
use minio::s3::{Client as MinioClient, ClientBuilder as MinioClientBuilder};

use nvisy_core::error::Error;
use nvisy_pipeline::provider::Provider;
use crate::client::{GetResult, ListResult, ObjectStoreBox, ObjectStoreClient};

/// S3-compatible object store client.
///
/// Wraps the MinIO [`MinioClient`] and scopes all operations to a single bucket.
pub struct S3ObjectStoreClient {
    /// Underlying MinIO client.
    client: MinioClient,
    /// Target S3 bucket name.
    bucket: String,
}

impl S3ObjectStoreClient {
    /// Create a new client bound to the given `bucket`.
    pub fn new(client: MinioClient, bucket: String) -> Self {
        Self { client, bucket }
    }
}

#[async_trait::async_trait]
impl ObjectStoreClient for S3ObjectStoreClient {
    async fn list(&self, prefix: &str, cursor: Option<&str>) -> Result<ListResult, Box<dyn std::error::Error + Send + Sync>> {
        use futures::StreamExt;

        let mut builder = self.client
            .list_objects(&self.bucket)
            .recursive(true)
            .use_api_v1(false);

        if !prefix.is_empty() {
            builder = builder.prefix(Some(prefix.to_string()));
        }

        if let Some(token) = cursor {
            builder = builder.continuation_token(Some(token.to_string()));
        }

        let mut stream = builder.to_stream().await;

        // Fetch one page
        if let Some(result) = stream.next().await {
            let resp = result?;
            let keys: Vec<String> = resp.contents
                .iter()
                .filter(|entry| !entry.is_prefix)
                .map(|entry| entry.name.clone())
                .collect();

            let next_cursor = resp.next_continuation_token.clone();

            Ok(ListResult { keys, next_cursor })
        } else {
            Ok(ListResult { keys: vec![], next_cursor: None })
        }
    }

    async fn get(&self, key: &str) -> Result<GetResult, Box<dyn std::error::Error + Send + Sync>> {
        let resp = self.client
            .get_object(&self.bucket, key)
            .send()
            .await?;

        let data = resp.content.to_segmented_bytes().await?.to_bytes();

        Ok(GetResult { data, content_type: None })
    }

    async fn put(&self, key: &str, data: Bytes, content_type: Option<&str>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let content = minio::s3::builders::ObjectContent::from(data);
        let mut builder = self.client
            .put_object_content(&self.bucket, key, content);

        if let Some(ct) = content_type {
            builder = builder.content_type(ct.to_string());
        }

        builder.send().await?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.client
            .delete_object(&self.bucket, key)
            .send()
            .await?;
        Ok(())
    }
}

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

fn default_region() -> String { "us-east-1".to_string() }

/// Factory that creates [`S3ObjectStoreClient`] instances from typed credentials.
pub struct S3Provider;

#[async_trait::async_trait]
impl Provider for S3Provider {
    type Credentials = S3Credentials;
    type Client = ObjectStoreBox;

    fn id(&self) -> &str { "s3" }

    fn validate_credentials(&self, _creds: &Self::Credentials) -> Result<(), Error> {
        Ok(())
    }

    async fn verify(&self, creds: &Self::Credentials) -> Result<(), Error> {
        self.validate_credentials(creds)?;
        Ok(())
    }

    async fn connect(&self, creds: &Self::Credentials) -> Result<Self::Client, Error> {
        let endpoint = creds.endpoint.as_deref().unwrap_or("https://s3.amazonaws.com");

        let mut base_url: BaseUrl = endpoint.parse().map_err(|e| {
            Error::runtime(format!("invalid endpoint URL: {e}"), "s3/connect", true)
        })?;
        base_url.region = creds.region.clone();

        let mut builder = MinioClientBuilder::new(base_url);

        // If access_key and secret_key provided, use static credentials
        if let (Some(access_key), Some(secret_key)) = (&creds.access_key_id, &creds.secret_access_key) {
            let provider = StaticProvider::new(
                access_key,
                secret_key,
                creds.session_token.as_deref(),
            );
            builder = builder.provider(Some(Box::new(provider)));
        }

        let client = builder.build().map_err(|e| {
            Error::runtime(format!("failed to build MinIO client: {e}"), "s3/connect", true)
        })?;

        let store_client = S3ObjectStoreClient::new(client, creds.bucket.clone());

        Ok(ObjectStoreBox::new(store_client))
    }
}

//! AWS S3 (and S3-compatible) provider implementation.
//!
//! Provides [`S3ObjectStoreClient`] which implements [`ObjectStoreClient`] and
//! [`S3ProviderFactory`] which plugs into the engine's provider system.

use aws_config::BehaviorVersion;
use aws_sdk_s3::Client as S3Client;
use bytes::Bytes;

use nvisy_core::error::Error;
use nvisy_core::traits::provider::{ConnectedInstance, ProviderFactory};
use crate::client::{GetResult, ListResult, ObjectStoreClient};

/// S3-compatible object store client.
///
/// Wraps the AWS SDK [`S3Client`] and scopes all operations to a single bucket.
pub struct S3ObjectStoreClient {
    /// Underlying AWS SDK client.
    client: S3Client,
    /// Target S3 bucket name.
    bucket: String,
}

impl S3ObjectStoreClient {
    /// Create a new client bound to the given `bucket`.
    pub fn new(client: S3Client, bucket: String) -> Self {
        Self { client, bucket }
    }
}

#[async_trait::async_trait]
impl ObjectStoreClient for S3ObjectStoreClient {
    async fn list(&self, prefix: &str, cursor: Option<&str>) -> Result<ListResult, Box<dyn std::error::Error + Send + Sync>> {
        let mut req = self.client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(prefix);

        if let Some(token) = cursor {
            req = req.continuation_token(token);
        }

        let resp = req.send().await?;

        let keys: Vec<String> = resp
            .contents()
            .iter()
            .filter_map(|obj| obj.key().map(|k| k.to_string()))
            .collect();

        let next_cursor = resp.next_continuation_token().map(|s| s.to_string());

        Ok(ListResult { keys, next_cursor })
    }

    async fn get(&self, key: &str) -> Result<GetResult, Box<dyn std::error::Error + Send + Sync>> {
        let resp = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;

        let content_type = resp.content_type().map(|s| s.to_string());
        let body = resp.body.collect().await?;
        let data = body.into_bytes();

        Ok(GetResult { data, content_type })
    }

    async fn put(&self, key: &str, data: Bytes, content_type: Option<&str>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut req = self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(data.into());

        if let Some(ct) = content_type {
            req = req.content_type(ct);
        }

        req.send().await?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        Ok(())
    }
}

/// Factory that creates [`S3ObjectStoreClient`] instances from JSON credentials.
///
/// Expected credential keys:
/// - `bucket` (required) -- S3 bucket name.
/// - `region` (optional, defaults to `us-east-1`).
/// - `endpoint` (optional) -- custom endpoint URL for S3-compatible services.
/// - `accessKeyId` / `secretAccessKey` / `sessionToken` (optional) -- static credentials.
pub struct S3ProviderFactory;

#[async_trait::async_trait]
impl ProviderFactory for S3ProviderFactory {
    fn id(&self) -> &str { "s3" }

    fn validate_credentials(&self, creds: &serde_json::Value) -> Result<(), Error> {
        let bucket = creds.get("bucket").and_then(|v| v.as_str());
        if bucket.is_none() {
            return Err(Error::validation("Missing 'bucket' in S3 credentials", "s3"));
        }
        Ok(())
    }

    async fn verify(&self, creds: &serde_json::Value) -> Result<(), Error> {
        self.validate_credentials(creds)?;
        // Could do a HeadBucket call here for verification
        Ok(())
    }

    async fn connect(&self, creds: &serde_json::Value) -> Result<ConnectedInstance, Error> {
        let bucket = creds.get("bucket")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::validation("Missing 'bucket'", "s3"))?
            .to_string();

        let region = creds.get("region")
            .and_then(|v| v.as_str())
            .unwrap_or("us-east-1");

        let endpoint = creds.get("endpoint")
            .and_then(|v| v.as_str());

        let mut config_loader = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_sdk_s3::config::Region::new(region.to_string()));

        // If access_key and secret_key provided, use static credentials
        if let (Some(access_key), Some(secret_key)) = (
            creds.get("accessKeyId").and_then(|v| v.as_str()),
            creds.get("secretAccessKey").and_then(|v| v.as_str()),
        ) {
            config_loader = config_loader.credentials_provider(
                aws_sdk_s3::config::Credentials::new(
                    access_key,
                    secret_key,
                    creds.get("sessionToken").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    None,
                    "nvisy-s3",
                ),
            );
        }

        let config = config_loader.load().await;
        let mut s3_config = aws_sdk_s3::config::Builder::from(&config);

        if let Some(ep) = endpoint {
            s3_config = s3_config.endpoint_url(ep).force_path_style(true);
        }

        let client = S3Client::from_conf(s3_config.build());
        let store_client = S3ObjectStoreClient::new(client, bucket);

        Ok(ConnectedInstance {
            client: Box::new(crate::client::ObjectStoreBox::new(store_client)),
            disconnect: None,
        })
    }
}

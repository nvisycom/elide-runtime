//! AWS S3 (and S3-compatible) provider implementation.
//!
//! Provides [`S3ObjectStoreClient`] which implements [`ObjectStoreClient`] and
//! [`S3ProviderFactory`] which plugs into the engine's provider system.

use aws_config::BehaviorVersion;
use aws_sdk_s3::Client as S3Client;
use bytes::Bytes;
use serde::Deserialize;

use nvisy_core::error::Error;
use nvisy_core::registry::provider::{ConnectedInstance, ProviderFactory};
use crate::client::{GetResult, ListResult, ObjectStoreBox, ObjectStoreClient};

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

/// Typed credentials for S3 provider.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct S3Credentials {
    /// S3 bucket name.
    pub bucket: String,
    /// AWS region (defaults to `us-east-1`).
    #[serde(default = "default_region")]
    pub region: String,
    /// Custom endpoint URL for S3-compatible services.
    #[serde(default)]
    pub endpoint: Option<String>,
    /// AWS access key ID for static credentials.
    #[serde(default)]
    pub access_key_id: Option<String>,
    /// AWS secret access key for static credentials.
    #[serde(default)]
    pub secret_access_key: Option<String>,
    /// AWS session token for temporary credentials.
    #[serde(default)]
    pub session_token: Option<String>,
}

fn default_region() -> String { "us-east-1".to_string() }

/// Factory that creates [`S3ObjectStoreClient`] instances from typed credentials.
pub struct S3ProviderFactory;

#[async_trait::async_trait]
impl ProviderFactory for S3ProviderFactory {
    type Credentials = S3Credentials;
    type Client = ObjectStoreBox;

    fn id(&self) -> &str { "s3" }

    fn validate_credentials(&self, _creds: &Self::Credentials) -> Result<(), Error> {
        // Bucket is required by the struct, so if we got here it's present.
        Ok(())
    }

    async fn verify(&self, creds: &Self::Credentials) -> Result<(), Error> {
        self.validate_credentials(creds)?;
        // Could do a HeadBucket call here for verification
        Ok(())
    }

    async fn connect(&self, creds: &Self::Credentials) -> Result<ConnectedInstance<Self::Client>, Error> {
        let mut config_loader = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_sdk_s3::config::Region::new(creds.region.clone()));

        // If access_key and secret_key provided, use static credentials
        if let (Some(access_key), Some(secret_key)) = (&creds.access_key_id, &creds.secret_access_key) {
            config_loader = config_loader.credentials_provider(
                aws_sdk_s3::config::Credentials::new(
                    access_key,
                    secret_key,
                    creds.session_token.clone(),
                    None,
                    "nvisy-s3",
                ),
            );
        }

        let config = config_loader.load().await;
        let mut s3_config = aws_sdk_s3::config::Builder::from(&config);

        if let Some(ref ep) = creds.endpoint {
            s3_config = s3_config.endpoint_url(ep).force_path_style(true);
        }

        let client = S3Client::from_conf(s3_config.build());
        let store_client = S3ObjectStoreClient::new(client, creds.bucket.clone());

        Ok(ConnectedInstance {
            client: ObjectStoreBox::new(store_client),
            disconnect: None,
        })
    }
}

//! Provider trait and object storage provider factories.

mod provider;
mod azure;
mod gcs;
mod s3;

pub use provider::Provider;
pub use azure::{AzureCredentials, AzureProvider};
pub use gcs::{GcsCredentials, GcsProvider};
pub use s3::{S3Credentials, S3Provider};

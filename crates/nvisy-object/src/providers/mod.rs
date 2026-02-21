//! Object storage provider factories.

mod azure;
mod gcs;
mod s3;

pub use azure::{AzureCredentials, AzureProvider};
pub use gcs::{GcsCredentials, GcsProvider};
pub use s3::{S3Credentials, S3Provider};

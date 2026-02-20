//! Convenience re-exports.

pub use crate::client::ObjectStoreClient;
pub use crate::error::ObjectStoreError;
pub use crate::providers::azure::AzureProvider;
pub use crate::providers::gcs::GcsProvider;
pub use crate::providers::s3::S3Provider;
pub use crate::streams::ObjectReadStream;
pub use crate::streams::ObjectWriteStream;
pub use crate::streams::{StreamSource, StreamTarget};

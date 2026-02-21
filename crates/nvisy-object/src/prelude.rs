//! Convenience re-exports.

pub use crate::providers::Provider;
pub use crate::streams::{StreamSource, StreamTarget};

pub use crate::client::{GetResult, ObjectStoreClient, PutResult};
pub use crate::providers::{AzureProvider, GcsProvider, S3Provider};
pub use crate::streams::{ObjectReadStream, ObjectWriteStream};

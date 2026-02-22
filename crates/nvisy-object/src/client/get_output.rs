//! Result type for [`ObjectStoreClient::get`](super::ObjectStoreClient::get).

use bytes::Bytes;
use object_store::ObjectMeta;

/// Result of a successful [`ObjectStoreClient::get`](super::ObjectStoreClient::get) call.
#[derive(Debug)]
pub struct GetOutput {
    /// Raw bytes of the retrieved object.
    pub data: Bytes,
    /// MIME content-type, if the backend provides one.
    pub content_type: Option<String>,
    /// Object metadata (size, etag, last_modified, location).
    pub meta: ObjectMeta,
}

//! Result type for [`ObjectStoreClient::put`](super::ObjectStoreClient::put) and
//! [`ObjectStoreClient::put_opts`](super::ObjectStoreClient::put_opts).

/// Result of a successful put operation.
#[derive(Debug)]
pub struct PutOutput {
    /// Unique identifier for the newly created object, if the backend provides one.
    pub e_tag: Option<String>,
    /// A version indicator for the newly created object, if the backend provides one.
    pub version: Option<String>,
}

impl From<object_store::PutResult> for PutOutput {
    fn from(r: object_store::PutResult) -> Self {
        Self {
            e_tag: r.e_tag,
            version: r.version,
        }
    }
}

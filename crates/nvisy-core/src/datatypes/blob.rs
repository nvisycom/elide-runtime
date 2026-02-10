//! Binary large object type and helpers.

use std::collections::HashMap;

use bytes::Bytes;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use super::Data;

/// Content type information for a blob.
///
/// Tracks both the caller-supplied MIME type and the type detected
/// from the file's magic bytes so consumers can choose the most
/// reliable value.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BlobContentInfo {
    /// MIME type provided by the caller (e.g. from HTTP Content-Type header).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    /// MIME type detected from magic bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_mime: Option<String>,
}

/// A binary large object flowing through the pipeline.
///
/// Blobs carry raw byte content along with an artifact registry
/// for derived data produced during pipeline processing. Each
/// pipeline action may attach artifacts (entities, documents,
/// redactions, etc.) to the blob as it passes through.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Blob {
    /// Common data-item fields (id, parent_id, metadata).
    #[serde(flatten)]
    pub data: Data,
    /// Storage path or key identifying this blob's origin.
    pub path: String,
    /// Raw byte content of the blob.
    #[serde(with = "bytes_serde")]
    #[cfg_attr(feature = "schema", schemars(with = "Vec<u8>"))]
    pub content: Bytes,
    /// Caller-supplied and auto-detected MIME type information.
    pub provided: BlobContentInfo,
    /// Artifacts derived from this blob during pipeline processing.
    ///
    /// Keys are artifact type names (e.g. `"documents"`, `"entities"`, `"redactions"`).
    /// Values are lists of JSON-serialized artifacts. Use [`add_artifact`] and
    /// [`get_artifacts`] for type-safe access.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub artifacts: HashMap<String, Vec<serde_json::Value>>,
}

impl Blob {
    /// Create a new blob from a storage path and raw content bytes.
    ///
    /// The MIME type is auto-detected from magic bytes when possible.
    pub fn new(path: impl Into<String>, content: impl Into<Bytes>) -> Self {
        let content = content.into();
        let detected_mime = infer::get(&content).map(|t| t.mime_type().to_string());
        Self {
            data: Data::new(),
            path: path.into(),
            content,
            provided: BlobContentInfo {
                mime: None,
                detected_mime,
            },
            artifacts: HashMap::new(),
        }
    }

    /// Set the caller-provided MIME type (builder pattern).
    pub fn with_content_type(mut self, mime: impl Into<String>) -> Self {
        self.provided.mime = Some(mime.into());
        self
    }

    /// Get the best-available MIME type (provided takes precedence over detected).
    pub fn content_type(&self) -> Option<&str> {
        self.provided
            .mime
            .as_deref()
            .or(self.provided.detected_mime.as_deref())
    }

    /// Get the file extension from the path.
    pub fn extension(&self) -> Option<&str> {
        self.path.rsplit('.').next()
    }

    /// Store a serializable artifact under the given key.
    pub fn add_artifact<T: Serialize>(&mut self, key: &str, value: &T) -> Result<(), serde_json::Error> {
        let json = serde_json::to_value(value)?;
        self.artifacts.entry(key.to_string()).or_default().push(json);
        Ok(())
    }

    /// Retrieve all artifacts under the given key, deserializing into `T`.
    pub fn get_artifacts<T: DeserializeOwned>(&self, key: &str) -> Result<Vec<T>, serde_json::Error> {
        match self.artifacts.get(key) {
            Some(values) => values.iter().map(|v| serde_json::from_value(v.clone())).collect(),
            None => Ok(Vec::new()),
        }
    }

    /// Check if any artifacts exist under the given key.
    pub fn has_artifacts(&self, key: &str) -> bool {
        self.artifacts.get(key).is_some_and(|v| !v.is_empty())
    }
}

pub(crate) mod bytes_serde {
    use bytes::Bytes;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(bytes.len()))?;
        for b in bytes.iter() {
            seq.serialize_element(b)?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v: Vec<u8> = Vec::deserialize(deserializer)?;
        Ok(Bytes::from(v))
    }
}

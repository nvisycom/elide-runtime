use bytes::Bytes;
use serde::{Deserialize, Serialize};
use crate::data::DataItem;

/// Content type information for a blob.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlobContentInfo {
    /// MIME type provided by the caller (e.g. from HTTP Content-Type header).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    /// MIME type detected from magic bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_mime: Option<String>,
}

/// A binary object from storage (file content + path + content type).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blob {
    #[serde(flatten)]
    pub data: DataItem,
    pub path: String,
    #[serde(with = "bytes_serde")]
    pub content: Bytes,
    pub provided: BlobContentInfo,
}

impl Blob {
    pub fn new(path: impl Into<String>, content: impl Into<Bytes>) -> Self {
        let content = content.into();
        let detected_mime = infer::get(&content).map(|t| t.mime_type().to_string());
        Self {
            data: DataItem::new(),
            path: path.into(),
            content,
            provided: BlobContentInfo {
                mime: None,
                detected_mime,
            },
        }
    }

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

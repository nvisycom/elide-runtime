//! [`Document`]: in-memory carrier for a decoder input.

use bytes::Bytes;
use hipstr::HipStr;
use uuid::Uuid;

/// A document ready for the codec.
///
/// Raw bytes plus the hints the codec needs to resolve a decoder,
/// plus a per-document correlation id the orchestrator threads
/// into tracing spans.
///
/// In-memory carrier: not persisted, not on the wire. Built from
/// a [`FileMetadata`] + its bytes when the engine reads a stored
/// file, or from an upload's body + headers at ingest.
///
/// [`FileMetadata`]: super::FileMetadata
#[derive(Debug, Clone)]
pub struct Document {
    /// Raw file bytes.
    pub bytes: Bytes,
    /// File extension the codec registry resolves on.
    ///
    /// E.g. `"txt"`, `"pdf"`, `"png"`. Case-insensitive, no
    /// leading dot.
    pub extension: HipStr<'static>,
    /// Caller-supplied MIME hint (e.g. `application/pdf`).
    ///
    /// The codec uses [`extension`], not this; recorded for audit
    /// and for clients that round-trip metadata.
    ///
    /// [`extension`]: Document::extension
    pub content_type: Option<HipStr<'static>>,
    /// Per-document correlation id.
    ///
    /// Threaded into tracing spans on the analyze + apply paths
    /// so a document can be traced end-to-end across the
    /// pipeline. Caller-minted (typically a per-run or
    /// per-request id). Defaults to a fresh UUIDv7 via
    /// [`Document::new`].
    pub correlation_id: Uuid,
}

impl Document {
    /// New document with a freshly minted UUIDv7 correlation id.
    ///
    /// Chain [`with_content_type`](Self::with_content_type) or
    /// [`with_correlation_id`](Self::with_correlation_id) to set
    /// the optional fields.
    pub fn new(bytes: impl Into<Bytes>, extension: impl Into<HipStr<'static>>) -> Self {
        Self {
            bytes: bytes.into(),
            extension: extension.into(),
            content_type: None,
            correlation_id: Uuid::now_v7(),
        }
    }

    /// Attach a caller-supplied MIME hint.
    #[must_use]
    pub fn with_content_type(mut self, content_type: impl Into<HipStr<'static>>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Override the auto-generated correlation id.
    ///
    /// Use when the caller already has an id to thread (a run id,
    /// a request id) and wants the pipeline's tracing to line up
    /// with it.
    #[must_use]
    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = correlation_id;
        self
    }
}

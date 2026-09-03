//! [`Document`]: in-memory carrier for a decoder input.

use std::path::Path;

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
    /// What to call this document.
    ///
    /// The engine keys the document's own content under this name
    /// and prefixes every part beneath it, so it is the first
    /// segment of every part path an audit reports:
    /// `["report.docx", "word/media/image1.png"]`. A real filename
    /// reads best.
    ///
    /// Must be unique within one analyze call. Two documents
    /// sharing a name is rejected rather than silently merged,
    /// since their parts would collide.
    pub name: HipStr<'static>,
    /// Raw file bytes.
    pub bytes: Bytes,
    /// File extension the codec registry resolves on, overriding
    /// the one [`name`] carries.
    ///
    /// E.g. `"txt"`, `"pdf"`, `"png"`. Case-insensitive, no
    /// leading dot. `None` resolves the format from [`name`], which
    /// is right whenever the name is a real filename.
    ///
    /// Set it when the name cannot be trusted to say what the bytes
    /// are: an upload whose client-supplied filename disagrees with
    /// its sniffed type, a name with no extension at all, or a
    /// dotfile like `.rels` whose leading dot is not one.
    ///
    /// [`name`]: Document::name
    pub extension: Option<HipStr<'static>>,
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
    /// New document named `name`, with a freshly minted UUIDv7
    /// correlation id.
    ///
    /// `name` is what the engine calls this document in every part
    /// path it reports, *and* what the format resolves from: pass a
    /// real filename (`report.docx`) and the codec follows. When the
    /// name cannot say what the bytes are, chain
    /// [`with_extension`].
    ///
    /// Chain [`with_content_type`] or [`with_correlation_id`] to
    /// set the other optional fields.
    ///
    /// [`with_extension`]: Self::with_extension
    /// [`with_content_type`]: Self::with_content_type
    /// [`with_correlation_id`]: Self::with_correlation_id
    pub fn new(name: impl Into<HipStr<'static>>, bytes: impl Into<Bytes>) -> Self {
        Self {
            name: name.into(),
            bytes: bytes.into(),
            extension: None,
            content_type: None,
            correlation_id: Uuid::now_v7(),
        }
    }

    /// Resolve the format from `extension` rather than from
    /// [`name`](Self::name).
    ///
    /// The explicit one always wins, whatever the name looks like.
    #[must_use]
    pub fn with_extension(mut self, extension: impl Into<HipStr<'static>>) -> Self {
        self.extension = Some(extension.into());
        self
    }

    /// The extension the codec resolves on: the explicit one, else
    /// the one [`name`](Self::name) carries.
    ///
    /// Lowercased, and `None` when neither supplies one — a name
    /// like `upload` or a dotfile like `.rels`, where
    /// [`Path::extension`] correctly finds nothing.
    ///
    /// [`Path::extension`]: std::path::Path::extension
    #[must_use]
    pub fn resolved_extension(&self) -> Option<String> {
        if let Some(explicit) = &self.extension {
            return Some(explicit.as_str().to_ascii_lowercase());
        }
        Path::new(self.name.as_str())
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
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

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::Document;

    #[test]
    fn the_format_resolves_from_the_name_unless_overridden() {
        let doc = Document::new("report.DOCX", Bytes::new());
        assert_eq!(
            doc.resolved_extension().as_deref(),
            Some("docx"),
            "a real filename needs no second source of truth, and case is not one",
        );

        // An upload whose client-supplied name disagrees with its
        // sniffed type: the explicit extension wins.
        let sniffed = Document::new("report.txt", Bytes::new()).with_extension("csv");
        assert_eq!(sniffed.resolved_extension().as_deref(), Some("csv"));

        // Nothing to resolve from: a name with no extension, and a
        // dotfile whose leading dot is not one. `decode` turns this
        // into a MalformedInput rather than guessing.
        assert_eq!(
            Document::new("upload", Bytes::new()).resolved_extension(),
            None
        );
        assert_eq!(
            Document::new(".rels", Bytes::new()).resolved_extension(),
            None
        );
    }
}

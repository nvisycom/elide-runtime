//! Plain-text handler: holds loaded text content and provides
//! location-based access via [`Handler`] + [`TextHandler`].
//!
//! The handler stores the text as a vector of lines together with a
//! trailing-newline flag so the original file can be reconstructed
//! byte-for-byte after edits.
//!
//! [`TextHandler::locations`] yields one [`TextLocation`] per line;
//! [`TextHandler::read`] returns the line at a given location;
//! [`TextHandler::redact`] applies redactions in place, mutating the
//! affected lines directly.

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::{DocumentType, TextFormat};
use nvisy_ontology::entity::TextLocation;

use nvisy_codec::handler::{TextRedaction, apply_text_redaction};
use nvisy_codec::document::{Located, LocationStream};
use nvisy_codec::handler::TextData;
use nvisy_codec::handler::{Handler, TextHandler};

const TARGET: &str = "txt-handler";

/// Handler for loaded plain-text content.
///
/// Each line is independently addressable via [`TextLocation`].
#[derive(Debug)]
pub struct TxtHandler {
    source: ContentSource,
    lines: Vec<String>,
    trailing_newline: bool,
}

impl Handler for TxtHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Text(TextFormat::Txt)
    }

    fn source(&self) -> ContentSource {
        self.source
    }

    #[tracing::instrument(name = "txt.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<ContentData, Error> {
        let mut out = self.lines.join("\n");
        if self.trailing_newline && !self.lines.is_empty() {
            out.push('\n');
        }
        let bytes = out.into_bytes();
        tracing::Span::current().record("output_bytes", bytes.len());
        let source = ContentSource::new().with_parent(&self.source);
        Ok(ContentData::new(source, bytes.into()))
    }
}

#[async_trait::async_trait]
impl TextHandler for TxtHandler {
    fn locations(&self) -> LocationStream<'_, TextLocation> {
        let source = self.source;
        let items: Vec<_> = self
            .line_offsets()
            .into_iter()
            .enumerate()
            .map(|(i, (start, end))| {
                Located::new(
                    source,
                    TextLocation {
                        start_offset: start,
                        end_offset: end,
                        line_number: Some((i + 1) as u32),
                        ..Default::default()
                    },
                )
            })
            .collect();
        LocationStream::new(futures::stream::iter(items))
    }

    async fn read(&self, location: &TextLocation) -> Option<TextData> {
        let offsets = self.line_offsets();
        let line_idx = offsets.iter().position(|&(start, end)| {
            location.start_offset >= start && location.end_offset <= end
        })?;
        let line = self.lines.get(line_idx)?;
        let line_start = offsets[line_idx].0;
        let local_start = location.start_offset - line_start;
        let local_end = location.end_offset - line_start;
        line.get(local_start..local_end).map(TextData::from)
    }

    async fn redact_at(
        &mut self,
        location: &TextLocation,
        redaction: TextRedaction,
    ) -> Result<(), Error> {
        let offsets = self.line_offsets();
        let Some(line_idx) = offsets
            .iter()
            .position(|&(start, end)| location.start_offset >= start && location.end_offset <= end)
        else {
            return Ok(());
        };
        let line_start = offsets[line_idx].0;
        let start = location.start_offset - line_start;
        let end = location.end_offset - line_start;
        apply_text_redaction(&mut self.lines[line_idx], &redaction, start, end, TARGET)
    }
}

impl TxtHandler {
    /// Create a new handler from lines and a trailing-newline flag.
    pub fn new(lines: Vec<String>, trailing_newline: bool) -> Self {
        Self {
            source: ContentSource::new(),
            lines,
            trailing_newline,
        }
    }

    /// Set the content source for lineage tracking.
    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }

    /// All lines in the document.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// A specific line by 0-based index.
    pub fn line(&self, index: usize) -> Option<&str> {
        self.lines.get(index).map(|s| s.as_str())
    }

    /// Whether the original source had a trailing newline.
    pub fn trailing_newline(&self) -> bool {
        self.trailing_newline
    }

    /// Total number of lines.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether the document has no lines.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Compute `(start_offset, end_offset)` for each line.
    fn line_offsets(&self) -> Vec<(usize, usize)> {
        let mut offset = 0;
        self.lines
            .iter()
            .map(|line| {
                let start = offset;
                let end = start + line.len();
                offset = end + 1;
                (start, end)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use nvisy_core::Error;

    use super::*;
    use nvisy_codec::handler::{ConflictPolicy, Redactions, TextHandler, TextOutput};

    fn handler(text: &str) -> TxtHandler {
        let trailing_newline = text.ends_with('\n');
        let lines = text.lines().map(String::from).collect();
        TxtHandler::new(lines, trailing_newline)
    }

    #[tokio::test]
    async fn locations_multiline() {
        let h = handler("hello\nworld\n");
        let items: Vec<_> = h.locations().collect().await;

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].location.start_offset, 0);
        assert_eq!(items[0].location.end_offset, 5);
        assert_eq!(items[0].location.line_number, Some(1));
        assert_eq!(items[1].location.start_offset, 6);
        assert_eq!(items[1].location.end_offset, 11);
        assert_eq!(items[1].location.line_number, Some(2));
    }

    #[tokio::test]
    async fn locations_single_line_no_newline() {
        let h = handler("no newline");
        let items: Vec<_> = h.locations().collect().await;

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].location.start_offset, 0);
        assert_eq!(items[0].location.end_offset, 10);
        assert!(!h.trailing_newline());
    }

    #[tokio::test]
    async fn read_returns_line() {
        let h = handler("hello\nworld\n");
        let loc = TextLocation {
            start_offset: 6,
            end_offset: 11,
            ..Default::default()
        };
        assert_eq!(h.read(&loc).await.unwrap().as_str(), "world");
    }

    #[tokio::test]
    async fn read_cross_line_returns_none() {
        let h = handler("hello\nworld\n");
        let loc = TextLocation {
            start_offset: 3,
            end_offset: 8,
            ..Default::default()
        };
        assert!(h.read(&loc).await.is_none());
    }

    #[tokio::test]
    async fn redact_replaces_whole_line() -> Result<(), Error> {
        let mut h = handler("hello\nworld\n");
        let items: Vec<_> = h.locations().collect().await;
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            items[1].location.clone(),
            TextRedaction::new(TextOutput::replace("[REDACTED]")),
        )
        .unwrap();
        h.redact(rs).await?;
        assert_eq!(h.lines(), &["hello", "[REDACTED]"]);
        Ok(())
    }

    #[tokio::test]
    async fn redact_substring_within_line() -> Result<(), Error> {
        // Entity-shaped location: bytes 6..11 in "hello world" picks the
        // substring "world", which lives inside the single-line span.
        let mut h = handler("hello world");
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            TextLocation {
                start_offset: 6,
                end_offset: 11,
                ..Default::default()
            },
            TextRedaction::new(TextOutput::replace("[X]")),
        )
        .unwrap();
        h.redact(rs).await?;
        assert_eq!(h.lines(), &["hello [X]"]);
        Ok(())
    }

    #[tokio::test]
    async fn redact_multiple_lines() -> Result<(), Error> {
        let mut h = handler("aaa\nbbb\nccc\n");
        let items: Vec<_> = h.locations().collect().await;
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            items[0].location.clone(),
            TextRedaction::new(TextOutput::replace("[X]")),
        )
        .unwrap();
        rs.try_insert(
            items[2].location.clone(),
            TextRedaction::new(TextOutput::replace("[Y]")),
        )
        .unwrap();
        h.redact(rs).await?;
        assert_eq!(h.lines(), &["[X]", "bbb", "[Y]"]);
        Ok(())
    }

    #[tokio::test]
    async fn redact_unknown_location_skipped() -> Result<(), Error> {
        let mut h = handler("one line");
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            TextLocation {
                start_offset: 999,
                end_offset: 1000,
                ..Default::default()
            },
            TextRedaction::new(TextOutput::replace("nope")),
        )
        .unwrap();
        h.redact(rs).await?;
        assert_eq!(h.lines(), &["one line"]);
        Ok(())
    }

    #[test]
    fn encode_with_trailing_newline() -> Result<(), Error> {
        let h = handler("hello\nworld\n");
        let content = h.encode()?;
        assert_eq!(content.as_bytes(), b"hello\nworld\n");
        Ok(())
    }

    #[test]
    fn encode_without_trailing_newline() -> Result<(), Error> {
        let h = handler("no newline");
        let content = h.encode()?;
        assert_eq!(content.as_bytes(), b"no newline");
        Ok(())
    }
}

//! Plain-text handler: holds loaded text content and provides
//! location-based access via [`Handler`] + [`Handle`].
//!
//! The handler stores the text as a vector of lines together with a
//! trailing-newline flag so the original file can be reconstructed
//! byte-for-byte after edits.
//!
//! [`Handle::locations`] yields one [`Text`] per line;
//! [`Handle::read`] returns the line at a given location;
//! [`Handle::redact`] applies redactions in place, mutating the
//! affected lines directly.

use nvisy_codec::core::{Handle, Located, LocationStream};
use nvisy_codec::handler::{Handler, TextData, TextRedaction};
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::{DocumentType, TextFormat};
use nvisy_ontology::modality::Text;

use super::redact;

const TARGET: &str = "txt-handler";

/// Handler for loaded plain-text content.
///
/// Each line is independently addressable via [`Text`].
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
impl Handle<Text> for TxtHandler {
    fn locations(&self) -> LocationStream<'_, Text> {
        let source = self.source;
        let items: Vec<_> = self
            .line_offsets()
            .into_iter()
            .map(|(start, end)| {
                Located::new(
                    source,
                    Text {
                        start,
                        end,
                        ..Default::default()
                    },
                )
            })
            .collect();
        LocationStream::new(futures::stream::iter(items))
    }

    async fn read(&self, location: &Text) -> Option<TextData> {
        let offsets = self.line_offsets();
        let line_idx = offsets
            .iter()
            .position(|&(start, end)| location.start >= start && location.end <= end)?;
        let line = self.lines.get(line_idx)?;
        let line_start = offsets[line_idx].0;
        let local_start = location.start - line_start;
        let local_end = location.end - line_start;
        line.get(local_start..local_end).map(TextData::from)
    }

    async fn redact_at(&mut self, location: &Text, redaction: TextRedaction) -> Result<(), Error> {
        let offsets = self.line_offsets();
        let Some(line_idx) = offsets
            .iter()
            .position(|&(start, end)| location.start >= start && location.end <= end)
        else {
            return Ok(());
        };
        let line_start = offsets[line_idx].0;
        let start = location.start - line_start;
        let end = location.end - line_start;
        let value = redaction.output().replacement_value().unwrap_or_default();
        redact::replace_range(&mut self.lines[line_idx], value, start, end, TARGET)
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

    /// Compute `(start, end)` for each line.
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
    use nvisy_codec::core::{Handle, Redactions};
    use nvisy_codec::handler::TextOutput;
    use nvisy_core::Error;

    use super::*;

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
        assert_eq!(items[0].location.start, 0);
        assert_eq!(items[0].location.end, 5);
        assert_eq!(items[1].location.start, 6);
        assert_eq!(items[1].location.end, 11);
    }

    #[tokio::test]
    async fn locations_single_line_no_newline() {
        let h = handler("no newline");
        let items: Vec<_> = h.locations().collect().await;

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].location.start, 0);
        assert_eq!(items[0].location.end, 10);
        assert!(!h.trailing_newline());
    }

    #[tokio::test]
    async fn read_returns_line() {
        let h = handler("hello\nworld\n");
        let loc = Text {
            start: 6,
            end: 11,
            ..Default::default()
        };
        assert_eq!(h.read(&loc).await.unwrap().as_str(), "world");
    }

    #[tokio::test]
    async fn read_cross_line_returns_none() {
        let h = handler("hello\nworld\n");
        let loc = Text {
            start: 3,
            end: 8,
            ..Default::default()
        };
        assert!(h.read(&loc).await.is_none());
    }

    #[tokio::test]
    async fn redact_replaces_whole_line() -> Result<(), Error> {
        let mut h = handler("hello\nworld\n");
        let items: Vec<_> = h.locations().collect().await;
        let mut rs = Redactions::new();
        rs.insert(
            items[1].location.clone(),
            TextRedaction::new(TextOutput::replace("[REDACTED]")),
        );
        h.redact(rs).await?;
        assert_eq!(h.lines(), &["hello", "[REDACTED]"]);
        Ok(())
    }

    #[tokio::test]
    async fn redact_substring_within_line() -> Result<(), Error> {
        // Entity-shaped location: bytes 6..11 in "hello world" picks the
        // substring "world", which lives inside the single-line span.
        let mut h = handler("hello world");
        let mut rs = Redactions::new();
        rs.insert(
            Text {
                start: 6,
                end: 11,
                ..Default::default()
            },
            TextRedaction::new(TextOutput::replace("[X]")),
        );
        h.redact(rs).await?;
        assert_eq!(h.lines(), &["hello [X]"]);
        Ok(())
    }

    #[tokio::test]
    async fn redact_multiple_lines() -> Result<(), Error> {
        let mut h = handler("aaa\nbbb\nccc\n");
        let items: Vec<_> = h.locations().collect().await;
        let mut rs = Redactions::new();
        rs.insert(
            items[0].location.clone(),
            TextRedaction::new(TextOutput::replace("[X]")),
        );
        rs.insert(
            items[2].location.clone(),
            TextRedaction::new(TextOutput::replace("[Y]")),
        );
        h.redact(rs).await?;
        assert_eq!(h.lines(), &["[X]", "bbb", "[Y]"]);
        Ok(())
    }

    #[tokio::test]
    async fn redact_unknown_location_skipped() -> Result<(), Error> {
        let mut h = handler("one line");
        let mut rs = Redactions::new();
        rs.insert(
            Text {
                start: 999,
                end: 1000,
                ..Default::default()
            },
            TextRedaction::new(TextOutput::replace("nope")),
        );
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

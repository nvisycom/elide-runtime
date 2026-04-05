//! Plain-text handler: holds loaded text content and provides
//! span-based access via [`Handler`] + [`TextHandler`].
//!
//! The handler stores the text as a vector of lines together with a
//! trailing-newline flag so the original file can be reconstructed
//! byte-for-byte after edits.
//!
//! # Span model
//!
//! [`TextHandler::text_spans`] yields one [`Span`] per line. Each span
//! is addressed by a [`TextLocation`] with byte offsets computed from
//! cumulative line lengths.
//!
//! [`TextHandler::edit_text`] replaces the content of lines at the
//! given locations.

use futures::StreamExt;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::{DocumentType, TextFormat};
use nvisy_ontology::entity::TextLocation;

use crate::document::{Span, SpanStream};
use crate::handler::text::TextData;
use crate::handler::{Handler, TextHandler};

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
    async fn text_spans(&self) -> SpanStream<'_, TextLocation, TextData> {
        SpanStream::new(futures::stream::iter(TxtSpanIter {
            lines: &self.lines,
            source: self.source,
            byte_offset: 0,
            index: 0,
        }))
    }

    async fn edit_text(
        &mut self,
        edits: SpanStream<'_, TextLocation, TextData>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        // Map each edit's byte offset range to a line index and apply.
        let offsets = self.line_offsets();
        for edit in edits {
            let line_idx = offsets
                .iter()
                .position(|&(start, _)| start == edit.id.start_offset)
                .ok_or_else(|| {
                    Error::validation(
                        format!("no line starts at byte offset {}", edit.id.start_offset),
                        "txt-handler",
                    )
                })?;
            self.lines[line_idx] = edit.data.into_inner();
        }
        Ok(())
    }

    async fn value_at(&self, location: &TextLocation) -> Option<String> {
        let offsets = self.line_offsets();
        let line_idx = offsets.iter().position(|&(start, end)| {
            location.start_offset >= start && location.end_offset <= end
        })?;
        let line = self.lines.get(line_idx)?;
        let line_start = offsets[line_idx].0;
        let local_start = location.start_offset - line_start;
        let local_end = location.end_offset - line_start;
        line.get(local_start..local_end).map(String::from)
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

/// Iterator over lines of a plain-text document, producing
/// [`TextLocation`]-addressed spans.
struct TxtSpanIter<'a> {
    lines: &'a [String],
    source: ContentSource,
    byte_offset: usize,
    index: usize,
}

impl<'a> Iterator for TxtSpanIter<'a> {
    type Item = Span<TextLocation, TextData>;

    fn next(&mut self) -> Option<Self::Item> {
        let line = self.lines.get(self.index)?;
        let start = self.byte_offset;
        let end = start + line.len();

        let location = TextLocation {
            start_offset: start,
            end_offset: end,
            line_number: Some((self.index + 1) as u32),
            ..Default::default()
        };
        let span = Span::new(location, TextData::from(line.clone())).with_source(self.source);

        // Advance past this line + newline separator.
        self.byte_offset = end + 1;
        self.index += 1;
        Some(span)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.lines.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for TxtSpanIter<'a> {}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use nvisy_core::Error;

    use super::*;
    use crate::document::Span;
    use crate::handler::TextHandler;

    fn handler(text: &str) -> TxtHandler {
        let trailing_newline = text.ends_with('\n');
        let lines = text.lines().map(String::from).collect();
        TxtHandler::new(lines, trailing_newline)
    }

    #[tokio::test]
    async fn view_spans_multiline() {
        let h = handler("hello\nworld\n");
        let spans: Vec<_> = h.text_spans().await.collect().await;

        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].id.start_offset, 0);
        assert_eq!(spans[0].id.end_offset, 5);
        assert_eq!(spans[0].id.line_number, Some(1));
        assert_eq!(spans[0].data, "hello");
        assert_eq!(spans[1].id.start_offset, 6);
        assert_eq!(spans[1].id.end_offset, 11);
        assert_eq!(spans[1].id.line_number, Some(2));
        assert_eq!(spans[1].data, "world");
    }

    #[tokio::test]
    async fn view_spans_single_line_no_newline() {
        let h = handler("no newline");
        let spans: Vec<_> = h.text_spans().await.collect().await;

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].data, "no newline");
        assert_eq!(spans[0].id.start_offset, 0);
        assert_eq!(spans[0].id.end_offset, 10);
        assert!(!h.trailing_newline());
    }

    #[tokio::test]
    async fn edit_spans_replace_line() -> Result<(), Error> {
        let mut h = handler("hello\nworld\n");
        let loc = TextLocation {
            start_offset: 6,
            end_offset: 11,
            ..Default::default()
        };
        h.edit_text(SpanStream::new(futures::stream::iter(vec![Span::new(
            loc,
            "[REDACTED]".into(),
        )])))
        .await?;
        assert_eq!(h.lines(), &["hello", "[REDACTED]"]);
        Ok(())
    }

    #[tokio::test]
    async fn edit_spans_bad_offset() {
        let mut h = handler("one line");
        let loc = TextLocation {
            start_offset: 999,
            end_offset: 1000,
            ..Default::default()
        };
        let err = h
            .edit_text(SpanStream::new(futures::stream::iter(vec![Span::new(
                loc,
                "nope".into(),
            )])))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no line starts at"));
    }

    #[tokio::test]
    async fn value_at_returns_line() {
        let h = handler("hello\nworld\n");
        let loc = TextLocation {
            start_offset: 6,
            end_offset: 11,
            ..Default::default()
        };
        assert_eq!(h.value_at(&loc).await, Some("world".to_string()));
    }

    #[tokio::test]
    async fn value_at_substring() {
        let h = handler("hello world");
        let loc = TextLocation {
            start_offset: 6,
            end_offset: 11,
            ..Default::default()
        };
        assert_eq!(h.value_at(&loc).await, Some("world".to_string()));
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

    #[tokio::test]
    async fn value_at_cross_line_returns_none() {
        let h = handler("hello\nworld\n");
        // Offsets spanning two lines should return None.
        let loc = TextLocation {
            start_offset: 3,
            end_offset: 8,
            ..Default::default()
        };
        assert_eq!(h.value_at(&loc).await, None);
    }

    #[tokio::test]
    async fn edit_multiple_lines() -> Result<(), Error> {
        let mut h = handler("aaa\nbbb\nccc\n");
        let spans: Vec<_> = h.text_spans().await.collect().await;
        h.edit_text(SpanStream::new(futures::stream::iter(vec![
            Span::new(spans[0].id.clone(), "[X]".into()),
            Span::new(spans[2].id.clone(), "[Y]".into()),
        ])))
        .await?;
        assert_eq!(h.lines(), &["[X]", "bbb", "[Y]"]);
        Ok(())
    }

    #[tokio::test]
    async fn empty_handler_spans() {
        let h = TxtHandler::new(vec![], false);
        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert!(spans.is_empty());
    }
}

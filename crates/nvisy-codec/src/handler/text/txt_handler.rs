//! Plain-text handler: holds loaded text content and provides
//! span-based access via [`Handler`] + [`TextHandler`].
//!
//! The handler stores the text as a vector of lines together with a
//! trailing-newline flag so the original file can be reconstructed
//! byte-for-byte after edits.
//!
//! # Span model
//!
//! [`TextHandler::text_spans`] yields one [`Span`] per line.  Each span
//! is addressed by a [`TxtSpan`] (0-based line index) and carries the
//! line content as a `String`.
//!
//! [`TextHandler::edit_text`] replaces the content of lines at the given
//! indices.

use futures::StreamExt;

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;

use crate::handler::{Handler, Span, SpanEditStream, SpanStream, TextHandler};
use crate::handler::text::TextData;

/// 0-based line index identifying a span within a plain-text document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TxtSpan(pub usize);

/// Handler for loaded plain-text content.
///
/// Each line is independently addressable via [`TxtSpan`].
#[derive(Debug)]
pub struct TxtHandler {
    pub(crate) source: ContentSource,
    pub(crate) lines: Vec<String>,
    pub(crate) trailing_newline: bool,
}

impl Handler for TxtHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Txt
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
    type TextId = TxtSpan;

    async fn text_spans(&self) -> SpanStream<'_, TxtSpan, TextData> {
        SpanStream::new(futures::stream::iter(TxtSpanIter {
            lines: &self.lines,
            index: 0,
        }))
    }

    async fn edit_text(
        &mut self,
        edits: SpanEditStream<'_, TxtSpan, TextData>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        for edit in edits {
            let line = self.lines.get_mut(edit.id.0).ok_or_else(|| {
                Error::validation(
                    format!("line index out of bounds: {}", edit.id.0),
                    "txt-handler",
                )
            })?;
            *line = edit.data.into_inner();
        }
        Ok(())
    }
}

impl TxtHandler {
    /// Create a new handler from lines and a trailing-newline flag.
    pub fn new(lines: Vec<String>, trailing_newline: bool) -> Self {
        Self { source: ContentSource::new(), lines, trailing_newline }
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
}

/// Iterator over lines of a plain-text document.
struct TxtSpanIter<'a> {
    lines: &'a [String],
    index: usize,
}

impl<'a> Iterator for TxtSpanIter<'a> {
    type Item = Span<TxtSpan, TextData>;

    fn next(&mut self) -> Option<Self::Item> {
        let line = self.lines.get(self.index)?;
        let span = Span::new(TxtSpan(self.index), TextData::from(line.clone()));
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
    use super::*;
    use crate::handler::{SpanEdit, TextHandler};
    use futures::StreamExt;
    use nvisy_core::Error;

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
        assert_eq!(spans[0].id, TxtSpan(0));
        assert_eq!(spans[0].data, "hello");
        assert_eq!(spans[1].id, TxtSpan(1));
        assert_eq!(spans[1].data, "world");
    }

    #[tokio::test]
    async fn view_spans_single_line_no_newline() {
        let h = handler("no newline");
        let spans: Vec<_> = h.text_spans().await.collect().await;

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].data, "no newline");
        assert!(!h.trailing_newline());
    }

    #[tokio::test]
    async fn edit_spans_replace_line() -> Result<(), Error> {
        let mut h = handler("hello\nworld\n");
        h.edit_text(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit::new(TxtSpan(1), "[REDACTED]".into()),
        ])))
        .await?;
        assert_eq!(h.lines(), &["hello", "[REDACTED]"]);
        Ok(())
    }

    #[tokio::test]
    async fn edit_spans_out_of_bounds() {
        let mut h = handler("one line");
        let err = h
            .edit_text(SpanEditStream::new(futures::stream::iter(vec![
                SpanEdit::new(TxtSpan(5), "nope".into()),
            ])))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
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

//! Plain-text handler — holds loaded text content and provides
//! span-based access via [`Handler`].
//!
//! The handler stores the text as a vector of lines together with a
//! trailing-newline flag so the original file can be reconstructed
//! byte-for-byte after edits.
//!
//! # Span model
//!
//! [`Handler::view_spans`] yields one [`Span`] per line.  Each span
//! is addressed by a [`TxtSpan`] (0-based line index) and carries the
//! line content as a `String`.
//!
//! [`Handler::edit_spans`] replaces the content of lines at the given
//! indices.

use futures::StreamExt;

use nvisy_core::error::Error;
use nvisy_core::fs::DocumentType;

use crate::document::{SpanEditStream, SpanStream};
use crate::handler::{Handler, Span};
use crate::transform::TextHandler;

/// 0-based line index identifying a span within a plain-text document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TxtSpan(pub usize);

/// Parsed plain-text content.
#[derive(Debug, Clone)]
pub struct TxtData {
    pub lines: Vec<String>,
    pub trailing_newline: bool,
}

/// Handler for loaded plain-text content.
///
/// Each line is independently addressable via [`TxtSpan`].
#[derive(Debug, Clone)]
pub struct TxtHandler {
    pub(crate) data: TxtData,
}

#[async_trait::async_trait]
impl Handler for TxtHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Txt
    }

    type SpanId = TxtSpan;
    type SpanData = String;

    async fn view_spans(&self) -> SpanStream<'_, TxtSpan, String> {
        SpanStream::new(futures::stream::iter(TxtSpanIter {
            lines: &self.data.lines,
            index: 0,
        }))
    }

    async fn edit_spans(
        &mut self,
        edits: SpanEditStream<'_, TxtSpan, String>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        for edit in edits {
            let line = self.data.lines.get_mut(edit.id.0).ok_or_else(|| {
                Error::validation(
                    format!("line index out of bounds: {}", edit.id.0),
                    "txt-handler",
                )
            })?;
            *line = edit.data;
        }
        Ok(())
    }
}

impl TxtHandler {
    /// Create a new handler from parsed text data.
    pub fn new(data: TxtData) -> Self {
        Self { data }
    }

    /// All lines in the document.
    pub fn lines(&self) -> &[String] {
        &self.data.lines
    }

    /// A specific line by 0-based index.
    pub fn line(&self, index: usize) -> Option<&str> {
        self.data.lines.get(index).map(|s| s.as_str())
    }

    /// Whether the original source had a trailing newline.
    pub fn trailing_newline(&self) -> bool {
        self.data.trailing_newline
    }

    /// Total number of lines.
    pub fn line_count(&self) -> usize {
        self.data.lines.len()
    }

    /// Consume the handler and return the inner [`TxtData`].
    pub fn into_data(self) -> TxtData {
        self.data
    }
}

impl TextHandler for TxtHandler {}

/// Iterator over lines of a plain-text document.
struct TxtSpanIter<'a> {
    lines: &'a [String],
    index: usize,
}

impl<'a> Iterator for TxtSpanIter<'a> {
    type Item = Span<TxtSpan, String>;

    fn next(&mut self) -> Option<Self::Item> {
        let line = self.lines.get(self.index)?;
        let span = Span {
            id: TxtSpan(self.index),
            data: line.clone(),
        };
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
    use crate::handler::SpanEdit;
    use futures::{Stream, StreamExt};

    fn handler(text: &str) -> TxtHandler {
        let trailing_newline = text.ends_with('\n');
        let lines = text.lines().map(String::from).collect();
        TxtHandler {
            data: TxtData {
                lines,
                trailing_newline,
            },
        }
    }

    #[tokio::test]
    async fn view_spans_multiline() {
        let h = handler("hello\nworld\n");
        let spans: Vec<_> = h.view_spans().await.collect().await;

        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].id, TxtSpan(0));
        assert_eq!(spans[0].data, "hello");
        assert_eq!(spans[1].id, TxtSpan(1));
        assert_eq!(spans[1].data, "world");
    }

    #[tokio::test]
    async fn view_spans_single_line_no_newline() {
        let h = handler("no newline");
        let spans: Vec<_> = h.view_spans().await.collect().await;

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].data, "no newline");
        assert!(!h.trailing_newline());
    }

    #[tokio::test]
    async fn view_spans_empty() {
        let h = handler("");
        let spans: Vec<_> = h.view_spans().await.collect().await;
        assert!(spans.is_empty());
    }

    #[tokio::test]
    async fn edit_spans_replace_line() {
        let mut h = handler("hello\nworld\n");
        h.edit_spans(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit {
                id: TxtSpan(1),
                data: "[REDACTED]".into(),
            },
        ])))
        .await
        .unwrap();
        assert_eq!(h.lines(), &["hello", "[REDACTED]"]);
    }

    #[tokio::test]
    async fn edit_spans_out_of_bounds() {
        let mut h = handler("one line");
        let err = h
            .edit_spans(SpanEditStream::new(futures::stream::iter(vec![
                SpanEdit {
                    id: TxtSpan(5),
                    data: "nope".into(),
                },
            ])))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
    }

    #[tokio::test]
    async fn view_spans_size_hint() {
        let h = handler("a\nb\nc\n");
        let stream = h.view_spans().await;
        assert_eq!(stream.size_hint(), (3, Some(3)));
    }
}

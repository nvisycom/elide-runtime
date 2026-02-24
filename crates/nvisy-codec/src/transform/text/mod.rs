//! Text rendering and redaction primitives.
//!
//! Provides the [`TextHandler`] async trait that text-bearing handlers
//! implement to support span-aware redaction.  The default implementation
//! groups redactions by [`SpanId`](Handler::SpanId), reads current content
//! via [`Handler::view_spans`], applies intra-span byte-offset replacements
//! right-to-left, and writes the results back via [`Handler::edit_spans`].

mod output;

pub use output::TextRedactionOutput;

use std::collections::HashMap;
use std::hash::Hash;

use futures::StreamExt;

use crate::document::SpanEditStream;
use crate::handler::{Handler, SpanEdit};
use nvisy_core::Error;

/// A located text redaction: pairs a span identifier and intra-span byte
/// range with a [`TextRedactionOutput`] that carries the replacement.
pub struct TextRedaction<S> {
    /// Which span this redaction targets.
    pub span_id: S,
    /// Byte offset where the redacted region starts within the span.
    pub start: usize,
    /// Byte offset where the redacted region ends (exclusive) within the span.
    pub end: usize,
    /// The redaction output that carries the replacement value.
    pub output: TextRedactionOutput,
}

/// Trait for handlers that support text redaction.
///
/// Extends [`Handler`] with [`redact_spans`](Self::redact_spans) which
/// applies a batch of span-aware text redactions.  The provided default
/// implementation groups redactions by span, reads content via
/// [`view_spans`](Handler::view_spans), applies byte-offset replacements
/// right-to-left per span, and writes back via
/// [`edit_spans`](Handler::edit_spans).
#[async_trait::async_trait]
pub trait TextHandler: Handler
where
    Self::SpanId: Eq + Hash,
    Self::SpanData: AsRef<str> + From<String>,
{
    /// Apply a batch of text redactions, mutating in place.
    ///
    /// Each [`TextRedaction`] identifies a span and an intra-span byte
    /// range together with a [`TextRedactionOutput`] whose replacement
    /// value is written into the content.  Replacements within each span
    /// are applied right-to-left so that byte offsets remain valid.
    async fn redact_spans(
        &mut self,
        redactions: &[TextRedaction<Self::SpanId>],
    ) -> Result<(), Error> {
        tracing::debug!(redaction_count = redactions.len(), "applying text redactions");
        if redactions.is_empty() {
            return Ok(());
        }

        // Group redactions by span id.
        let mut by_span: HashMap<&Self::SpanId, Vec<(usize, usize, String)>> = HashMap::new();
        for r in redactions {
            let value = r
                .output
                .replacement_value()
                .unwrap_or_default()
                .to_string();
            by_span
                .entry(&r.span_id)
                .or_default()
                .push((r.start, r.end, value));
        }

        // Read current content for affected spans.
        let all_spans: Vec<_> = self.view_spans().await.collect().await;

        let mut edits: Vec<SpanEdit<Self::SpanId, Self::SpanData>> = Vec::new();
        for span in &all_spans {
            let Some(replacements) = by_span.get_mut(&span.id) else {
                continue;
            };
            let content = span.data.as_ref();

            // Sort right-to-left so earlier byte offsets stay valid.
            replacements.sort_by(|a, b| b.0.cmp(&a.0));

            let mut result = content.to_string();
            for (start, end, value) in replacements.iter() {
                let s = (*start).min(result.len());
                let e = (*end).min(result.len());
                if s >= e {
                    continue;
                }
                if !result.is_char_boundary(s) || !result.is_char_boundary(e) {
                    return Err(Error::validation(
                        format!(
                            "redaction offset falls mid-character \
                             (start={start}, end={end}, len={})",
                            result.len()
                        ),
                        "text-handler",
                    ));
                }
                result.replace_range(s..e, value);
            }

            edits.push(SpanEdit::new(span.id.clone(), Self::SpanData::from(result)));
        }

        let edit_count = edits.len();
        if !edits.is_empty() {
            self.edit_spans(SpanEditStream::new(futures::stream::iter(edits)))
                .await?;
        }

        tracing::debug!(edit_count, "text redactions applied");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::{Handler, TxtHandler, TxtSpan};
    use futures::StreamExt;
    use nvisy_core::Error;

    fn handler(text: &str) -> TxtHandler {
        let trailing_newline = text.ends_with('\n');
        let lines = text.lines().map(String::from).collect();
        TxtHandler::new(lines, trailing_newline)
    }

    fn replace(span: usize, start: usize, end: usize, replacement: &str) -> TextRedaction<TxtSpan> {
        TextRedaction {
            span_id: TxtSpan(span),
            start,
            end,
            output: TextRedactionOutput::Replace {
                replacement: replacement.to_string(),
            },
        }
    }

    fn remove(span: usize, start: usize, end: usize) -> TextRedaction<TxtSpan> {
        TextRedaction {
            span_id: TxtSpan(span),
            start,
            end,
            output: TextRedactionOutput::Remove,
        }
    }

    #[tokio::test]
    async fn single_span_single_redaction() -> Result<(), Error> {
        let mut h = handler("hello world\n");
        TextHandler::redact_spans(&mut h, &[replace(0, 0, 5, "[NAME]")]).await?;

        let spans: Vec<_> = h.view_spans().await.collect().await;
        assert_eq!(spans[0].data, "[NAME] world");
        Ok(())
    }

    #[tokio::test]
    async fn multiple_redactions_within_one_span() -> Result<(), Error> {
        let mut h = handler("Alice met Bob\n");
        TextHandler::redact_spans(&mut h, &[
            replace(0, 0, 5, "[X]"),
            replace(0, 10, 13, "[Y]"),
        ])
        .await?;

        let spans: Vec<_> = h.view_spans().await.collect().await;
        assert_eq!(spans[0].data, "[X] met [Y]");
        Ok(())
    }

    #[tokio::test]
    async fn redaction_spanning_entire_content_replace() -> Result<(), Error> {
        let mut h = handler("secret\n");
        TextHandler::redact_spans(&mut h, &[replace(0, 0, 6, "[REDACTED]")]).await?;

        let spans: Vec<_> = h.view_spans().await.collect().await;
        assert_eq!(spans[0].data, "[REDACTED]");
        Ok(())
    }

    #[tokio::test]
    async fn redaction_spanning_entire_content_remove() -> Result<(), Error> {
        let mut h = handler("secret\n");
        TextHandler::redact_spans(&mut h, &[remove(0, 0, 6)]).await?;

        let spans: Vec<_> = h.view_spans().await.collect().await;
        assert_eq!(spans[0].data, "");
        Ok(())
    }

    #[tokio::test]
    async fn empty_redactions_is_noop() -> Result<(), Error> {
        let mut h = handler("unchanged\n");
        TextHandler::redact_spans(&mut h, &[]).await?;

        let spans: Vec<_> = h.view_spans().await.collect().await;
        assert_eq!(spans[0].data, "unchanged");
        Ok(())
    }

    #[tokio::test]
    async fn invalid_utf8_boundary_returns_error() {
        // "café" = [99, 97, 102, 195, 169] — byte 4 is the start of
        // the two-byte é, so byte 4 is a valid boundary but byte 4
        // splitting into the middle of é would be offset 4..5 where 5
        // is *not* a char boundary (it's the second byte of é).
        let mut h = handler("café\n");
        let err = TextHandler::redact_spans(&mut h, &[replace(0, 4, 5, "X")])
            .await
            .unwrap_err();
        assert!(err.to_string().contains("mid-character"));
    }

    #[tokio::test]
    async fn multiple_spans_with_separate_redactions() -> Result<(), Error> {
        let mut h = handler("hello\nworld\n");
        TextHandler::redact_spans(&mut h, &[
            replace(0, 0, 5, "[A]"),
            replace(1, 0, 5, "[B]"),
        ])
        .await?;

        let spans: Vec<_> = h.view_spans().await.collect().await;
        assert_eq!(spans[0].data, "[A]");
        assert_eq!(spans[1].data, "[B]");
        Ok(())
    }
}

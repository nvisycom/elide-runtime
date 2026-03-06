//! [`TextTransform`] async trait and blanket implementation.
//!
//! The blanket implementation groups redactions by
//! [`TextId`](TextHandler::TextId), reads current content via
//! [`TextHandler::text_spans`], applies intra-span byte-offset
//! replacements right-to-left, and writes the results back via
//! [`TextHandler::edit_text`].

use std::collections::HashMap;
use std::hash::Hash;

use futures::StreamExt;
use nvisy_core::Error;

use crate::document::{SpanEdit, SpanEditStream};
use crate::handler::{TextData, TextHandler};

use super::instruction::TextRedaction;

/// Extension trait for handlers that support text redaction.
///
/// Extends [`TextHandler`] with [`redact_text`](Self::redact_text) which
/// applies a batch of span-aware text redactions.
#[async_trait::async_trait]
pub trait TextTransform: TextHandler {
    /// Apply a batch of text redactions, mutating in place.
    ///
    /// Each [`TextRedaction`] identifies a span and an intra-span byte
    /// range together with a [`TextOutput`] whose replacement
    /// value is written into the content.  Replacements within each span
    /// are applied right-to-left so that byte offsets remain valid.
    async fn redact_text(
        &mut self,
        redactions: &[TextRedaction<Self::TextId>],
    ) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl<H: TextHandler> TextTransform for H
where
    H::TextId: Eq + Hash,
{
    async fn redact_text(
        &mut self,
        redactions: &[TextRedaction<Self::TextId>],
    ) -> Result<(), Error> {
        tracing::debug!(
            redaction_count = redactions.len(),
            "applying text redactions"
        );
        if redactions.is_empty() {
            return Ok(());
        }

        // Group redactions by span id.
        let mut by_span: HashMap<&Self::TextId, Vec<(usize, usize, String)>> = HashMap::new();
        for r in redactions {
            let value = r.output.replacement_value().unwrap_or_default().to_string();
            by_span
                .entry(&r.span_id)
                .or_default()
                .push((r.start, r.end, value));
        }

        // Read current content for affected spans.
        let all_spans: Vec<_> = self.text_spans().await.collect().await;

        let mut edits: Vec<SpanEdit<Self::TextId, TextData>> = Vec::new();
        for span in &all_spans {
            let Some(replacements) = by_span.get_mut(&span.id) else {
                continue;
            };
            let content: &str = span.data.as_ref();

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
                        "text-redact",
                    ));
                }
                result.replace_range(s..e, value);
            }

            edits.push(SpanEdit::new(span.id.clone(), TextData::from(result)));
        }

        let edit_count = edits.len();
        if !edits.is_empty() {
            self.edit_text(SpanEditStream::new(futures::stream::iter(edits)))
                .await?;
        }

        tracing::debug!(edit_count, "text redactions applied");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use nvisy_core::Result;

    use super::*;
    use crate::handler::{TxtHandler, TxtSpan};
    use crate::transform::TextOutput;

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
            output: TextOutput::Replace {
                replacement: replacement.to_string(),
            },
        }
    }

    fn remove(span: usize, start: usize, end: usize) -> TextRedaction<TxtSpan> {
        TextRedaction {
            span_id: TxtSpan(span),
            start,
            end,
            output: TextOutput::Remove,
        }
    }

    #[tokio::test]
    async fn single_span_single_redaction() -> Result<()> {
        let mut h = handler("hello world\n");
        TextTransform::redact_text(&mut h, &[replace(0, 0, 5, "[NAME]")]).await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "[NAME] world");
        Ok(())
    }

    #[tokio::test]
    async fn multiple_redactions_within_one_span() -> Result<()> {
        let mut h = handler("Alice met Bob\n");
        TextTransform::redact_text(
            &mut h,
            &[replace(0, 0, 5, "[X]"), replace(0, 10, 13, "[Y]")],
        )
        .await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "[X] met [Y]");
        Ok(())
    }

    #[tokio::test]
    async fn redaction_spanning_entire_content_replace() -> Result<()> {
        let mut h = handler("secret\n");
        TextTransform::redact_text(&mut h, &[replace(0, 0, 6, "[REDACTED]")]).await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "[REDACTED]");
        Ok(())
    }

    #[tokio::test]
    async fn redaction_spanning_entire_content_remove() -> Result<()> {
        let mut h = handler("secret\n");
        TextTransform::redact_text(&mut h, &[remove(0, 0, 6)]).await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "");
        Ok(())
    }

    #[tokio::test]
    async fn empty_redactions_is_noop() -> Result<()> {
        let mut h = handler("unchanged\n");
        TextTransform::redact_text(&mut h, &[]).await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "unchanged");
        Ok(())
    }

    #[tokio::test]
    async fn invalid_utf8_boundary_returns_error() {
        let mut h = handler("café\n");
        let err = TextTransform::redact_text(&mut h, &[replace(0, 4, 5, "X")])
            .await
            .unwrap_err();
        assert!(err.to_string().contains("mid-character"));
    }

    #[tokio::test]
    async fn multiple_spans_with_separate_redactions() -> Result<()> {
        let mut h = handler("hello\nworld\n");
        TextTransform::redact_text(&mut h, &[replace(0, 0, 5, "[A]"), replace(1, 0, 5, "[B]")])
            .await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "[A]");
        assert_eq!(spans[1].data, "[B]");
        Ok(())
    }
}

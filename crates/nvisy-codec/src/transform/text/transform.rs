//! [`TextTransform`] async trait and blanket implementation.
//!
//! The blanket implementation groups redactions by location, reads
//! current content via [`TextHandler::text_spans`], applies intra-span
//! byte-offset replacements right-to-left, and writes the results back
//! via [`TextHandler::edit_text`].

use std::cmp::Reverse;
use std::collections::HashMap;

use futures::StreamExt;
use nvisy_core::Error;
use nvisy_ontology::entity::TextLocation;

use super::instruction::TextRedaction;
use crate::document::{Span, SpanStream};
use crate::handler::{TextData, TextHandler};

const TARGET: &str = "nvisy_codec::transform::text";

/// Extension trait for handlers that support text redaction.
#[async_trait::async_trait]
pub trait TextTransform: TextHandler {
    /// Apply a batch of text redactions, mutating in place.
    ///
    /// Each [`TextRedaction`] identifies a span by [`TextLocation`] and
    /// an intra-span byte range with a replacement value. Replacements
    /// within each span are applied right-to-left so byte offsets
    /// remain valid.
    async fn redact_text(
        &mut self,
        redactions: &[TextRedaction<TextLocation>],
    ) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl<H: TextHandler> TextTransform for H {
    async fn redact_text(
        &mut self,
        redactions: &[TextRedaction<TextLocation>],
    ) -> Result<(), Error> {
        tracing::debug!(
            target: TARGET,
            redaction_count = redactions.len(),
            "applying text redactions"
        );
        if redactions.is_empty() {
            return Ok(());
        }

        // Group redactions by span start offset (each span has a unique start).
        let mut by_span: HashMap<usize, Vec<(usize, usize, String)>> = HashMap::new();
        for r in redactions {
            let value = r.output.replacement_value().unwrap_or_default().to_string();
            by_span
                .entry(r.span_id.start_offset)
                .or_default()
                .push((r.start, r.end, value));
        }

        // Read current content for affected spans.
        let all_spans: Vec<_> = self.text_spans().await.collect().await;

        let mut edits: Vec<Span<TextLocation, TextData>> = Vec::new();
        for span in &all_spans {
            let Some(replacements) = by_span.get_mut(&span.id.start_offset) else {
                continue;
            };
            let content: &str = span.data.as_ref();

            // Sort right-to-left so earlier byte offsets stay valid.
            replacements.sort_by_key(|r| Reverse(r.0));

            // Check for overlapping ranges (sorted descending by start).
            for pair in replacements.windows(2) {
                let (later_start, _, _) = &pair[0]; // higher start
                let (earlier_start, earlier_end, _) = &pair[1]; // lower start
                if *earlier_end > *later_start {
                    return Err(Error::validation(
                        format!(
                            "overlapping redaction ranges: {}..{} and {}..{}",
                            earlier_start, earlier_end, later_start, pair[0].1,
                        ),
                        "text-redact",
                    ));
                }
            }

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

            edits.push(Span::new(span.id.clone(), TextData::from(result)));
        }

        let edit_count = edits.len();
        if !edits.is_empty() {
            self.edit_text(SpanStream::new(futures::stream::iter(edits)))
                .await?;
        }

        tracing::debug!(target: TARGET, edit_count, "text redactions applied");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use nvisy_core::Result;

    use super::*;
    use crate::handler::TxtHandler;
    use crate::transform::TextOutput;

    fn handler(text: &str) -> TxtHandler {
        let trailing_newline = text.ends_with('\n');
        let lines = text.lines().map(String::from).collect();
        TxtHandler::new(lines, trailing_newline)
    }

    /// Build a text redaction targeting line `line_idx` at `start..end`.
    async fn replace_at(
        h: &TxtHandler,
        line_idx: usize,
        start: usize,
        end: usize,
        replacement: &str,
    ) -> TextRedaction<TextLocation> {
        let spans: Vec<_> = h.text_spans().await.collect().await;
        TextRedaction {
            span_id: spans[line_idx].id.clone(),
            start,
            end,
            output: TextOutput::Replace {
                replacement: replacement.to_string(),
            },
        }
    }

    async fn remove_at(
        h: &TxtHandler,
        line_idx: usize,
        start: usize,
        end: usize,
    ) -> TextRedaction<TextLocation> {
        let spans: Vec<_> = h.text_spans().await.collect().await;
        TextRedaction {
            span_id: spans[line_idx].id.clone(),
            start,
            end,
            output: TextOutput::Remove,
        }
    }

    #[tokio::test]
    async fn single_span_single_redaction() -> Result<()> {
        let mut h = handler("hello world\n");
        let r = replace_at(&h, 0, 0, 5, "[NAME]").await;
        TextTransform::redact_text(&mut h, &[r]).await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "[NAME] world");
        Ok(())
    }

    #[tokio::test]
    async fn multiple_redactions_within_one_span() -> Result<()> {
        let mut h = handler("Alice met Bob\n");
        let r1 = replace_at(&h, 0, 0, 5, "[X]").await;
        let r2 = replace_at(&h, 0, 10, 13, "[Y]").await;
        TextTransform::redact_text(&mut h, &[r1, r2]).await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "[X] met [Y]");
        Ok(())
    }

    #[tokio::test]
    async fn redaction_spanning_entire_content_replace() -> Result<()> {
        let mut h = handler("secret\n");
        let r = replace_at(&h, 0, 0, 6, "[REDACTED]").await;
        TextTransform::redact_text(&mut h, &[r]).await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "[REDACTED]");
        Ok(())
    }

    #[tokio::test]
    async fn redaction_spanning_entire_content_remove() -> Result<()> {
        let mut h = handler("secret\n");
        let r = remove_at(&h, 0, 0, 6).await;
        TextTransform::redact_text(&mut h, &[r]).await?;

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
    async fn multiple_spans_with_separate_redactions() -> Result<()> {
        let mut h = handler("hello\nworld\n");
        let r1 = replace_at(&h, 0, 0, 5, "[A]").await;
        let r2 = replace_at(&h, 1, 0, 5, "[B]").await;
        TextTransform::redact_text(&mut h, &[r1, r2]).await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "[A]");
        assert_eq!(spans[1].data, "[B]");
        Ok(())
    }
}

//! [`TextTransform`] async trait and blanket implementation.
//!
//! The blanket implementation walks the per-span groups in a
//! [`Redactions`] collection, reads current content via
//! [`TextHandler::text_spans`], applies intra-span byte-offset
//! replacements right-to-left (so earlier offsets stay valid), and
//! writes the results back via [`TextHandler::edit_text`].
//!
//! Overlap detection is owned by [`Redactions`]; this transform
//! trusts that ranges within a span do not overlap.

use std::cmp::Reverse;

use futures::StreamExt;
use nvisy_core::Error;
use nvisy_ontology::entity::TextLocation;

use super::instruction::TextRedaction;
use crate::document::{Span, SpanStream};
use crate::handler::{TextData, TextHandler};
use crate::transform::Redactions;

const TARGET: &str = "nvisy_codec::transform::text";

/// Extension trait for handlers that support text redaction.
#[async_trait::async_trait]
pub trait TextTransform: TextHandler {
    /// Apply a batch of text redactions, mutating in place.
    ///
    /// Redactions are grouped by [`TextLocation`] span in the input
    /// [`Redactions`] collection. The implementation assumes ranges
    /// within a single span do not overlap — the [`Redactions`]
    /// collection enforces this on insert.
    async fn redact_text(
        &mut self,
        redactions: Redactions<TextLocation, TextRedaction>,
    ) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl<H: TextHandler> TextTransform for H {
    async fn redact_text(
        &mut self,
        redactions: Redactions<TextLocation, TextRedaction>,
    ) -> Result<(), Error> {
        tracing::debug!(
            target: TARGET,
            redaction_count = redactions.len(),
            "applying text redactions"
        );
        if redactions.is_empty() {
            return Ok(());
        }

        // Read current content for all spans, then walk each affected span.
        let all_spans: Vec<_> = self.text_spans().await.collect().await;

        let mut edits: Vec<Span<TextLocation, TextData>> = Vec::new();
        for (span_loc, mut items) in redactions {
            let Some(span) = all_spans
                .iter()
                .find(|s| s.id.start_offset == span_loc.start_offset)
            else {
                continue;
            };
            let content: &str = span.data.as_ref();

            // Sort right-to-left so earlier byte offsets stay valid.
            items.sort_by_key(|r| Reverse(r.start));

            let mut result = content.to_string();
            for r in &items {
                let value = r.output.replacement_value().unwrap_or_default();
                let s = r.start.min(result.len());
                let e = r.end.min(result.len());
                if s >= e {
                    continue;
                }
                if !result.is_char_boundary(s) || !result.is_char_boundary(e) {
                    return Err(Error::validation(
                        format!(
                            "redaction offset falls mid-character \
                             (start={}, end={}, len={})",
                            r.start,
                            r.end,
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
    use crate::transform::{ConflictPolicy, TextOutput};

    fn handler(text: &str) -> TxtHandler {
        let trailing_newline = text.ends_with('\n');
        let lines = text.lines().map(String::from).collect();
        TxtHandler::new(lines, trailing_newline)
    }

    async fn span_for(h: &TxtHandler, line_idx: usize) -> TextLocation {
        let spans: Vec<_> = h.text_spans().await.collect().await;
        spans[line_idx].id.clone()
    }

    #[tokio::test]
    async fn single_span_single_redaction() -> Result<()> {
        let mut h = handler("hello world\n");
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            span_for(&h, 0).await,
            TextRedaction::new(0, 5, TextOutput::replace("[NAME]")),
        )
        .unwrap();
        TextTransform::redact_text(&mut h, rs).await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "[NAME] world");
        Ok(())
    }

    #[tokio::test]
    async fn multiple_redactions_within_one_span() -> Result<()> {
        let mut h = handler("Alice met Bob\n");
        let id = span_for(&h, 0).await;
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            id.clone(),
            TextRedaction::new(0, 5, TextOutput::replace("[X]")),
        )
        .unwrap();
        rs.try_insert(id, TextRedaction::new(10, 13, TextOutput::replace("[Y]")))
            .unwrap();
        TextTransform::redact_text(&mut h, rs).await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "[X] met [Y]");
        Ok(())
    }

    #[tokio::test]
    async fn redaction_spanning_entire_content_replace() -> Result<()> {
        let mut h = handler("secret\n");
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            span_for(&h, 0).await,
            TextRedaction::new(0, 6, TextOutput::replace("[REDACTED]")),
        )
        .unwrap();
        TextTransform::redact_text(&mut h, rs).await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "[REDACTED]");
        Ok(())
    }

    #[tokio::test]
    async fn redaction_spanning_entire_content_remove() -> Result<()> {
        let mut h = handler("secret\n");
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            span_for(&h, 0).await,
            TextRedaction::new(0, 6, TextOutput::Remove),
        )
        .unwrap();
        TextTransform::redact_text(&mut h, rs).await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "");
        Ok(())
    }

    #[tokio::test]
    async fn empty_redactions_is_noop() -> Result<()> {
        let mut h = handler("unchanged\n");
        let rs: Redactions<TextLocation, TextRedaction> = Redactions::default();
        TextTransform::redact_text(&mut h, rs).await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "unchanged");
        Ok(())
    }

    #[tokio::test]
    async fn multiple_spans_with_separate_redactions() -> Result<()> {
        let mut h = handler("hello\nworld\n");
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            span_for(&h, 0).await,
            TextRedaction::new(0, 5, TextOutput::replace("[A]")),
        )
        .unwrap();
        rs.try_insert(
            span_for(&h, 1).await,
            TextRedaction::new(0, 5, TextOutput::replace("[B]")),
        )
        .unwrap();
        TextTransform::redact_text(&mut h, rs).await?;

        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans[0].data, "[A]");
        assert_eq!(spans[1].data, "[B]");
        Ok(())
    }
}

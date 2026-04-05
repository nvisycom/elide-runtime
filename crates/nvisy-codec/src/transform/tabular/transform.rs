//! [`TabularTransform`] async trait and blanket implementation.
//!
//! Bridges [`TabularRedaction`] (cell-addressed by row/col) to the
//! underlying [`TextHandler`] (byte-offset-addressed spans).
//!
//! The blanket implementation collects text spans, builds a row/col
//! grid from `line_number`, maps each [`TabularRedaction`] to the
//! corresponding text span, applies intra-cell byte-offset
//! replacements right-to-left, and writes results back via
//! [`TextHandler::edit_text`].

use std::collections::HashMap;

use futures::StreamExt;
use nvisy_core::Error;
use nvisy_ontology::entity::TextLocation;

use super::instruction::TabularRedaction;
use crate::document::{Span, SpanStream};
use crate::handler::{TextData, TextHandler};

const TARGET: &str = "nvisy_codec::transform::tabular";

/// Extension trait for text handlers that support cell-addressed
/// tabular redaction.
///
/// Implemented automatically for all [`TextHandler`] types via a
/// blanket impl. The trait translates `(row, col)` cell addresses
/// into the byte-offset `TextLocation`s that the handler understands.
#[async_trait::async_trait]
pub trait TabularTransform: TextHandler {
    /// Apply a batch of cell-addressed redactions, mutating in place.
    ///
    /// Each [`TabularRedaction`] identifies a cell by
    /// [`TabularLocation`](nvisy_ontology::entity::TabularLocation)
    /// and an intra-cell byte range with a replacement value.
    async fn redact_tabular(&mut self, redactions: &[TabularRedaction]) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl<H: TextHandler> TabularTransform for H {
    async fn redact_tabular(&mut self, redactions: &[TabularRedaction]) -> Result<(), Error> {
        tracing::debug!(
            target: TARGET,
            redaction_count = redactions.len(),
            "applying tabular redactions"
        );
        if redactions.is_empty() {
            return Ok(());
        }

        // Collect all text spans and build a (row, col) -> span index.
        let all_spans: Vec<_> = self.text_spans().await.collect().await;

        // Group span indices by line_number (= row), preserving column order.
        let mut rows: HashMap<u32, Vec<usize>> = HashMap::new();
        for (idx, span) in all_spans.iter().enumerate() {
            let line = span.id.line_number.unwrap_or(1);
            rows.entry(line).or_default().push(idx);
        }

        // Build sorted row keys so we can map row_index -> line_number.
        let mut line_numbers: Vec<u32> = rows.keys().copied().collect();
        line_numbers.sort_unstable();

        // Group redactions by cell, collecting intra-cell replacements.
        let mut by_cell: HashMap<(usize, usize), Vec<(usize, usize, String)>> = HashMap::new();
        for r in redactions {
            let value = r.output.replacement_value().unwrap_or_default().to_string();
            by_cell
                .entry((r.location.row_index, r.location.column_index))
                .or_default()
                .push((r.start, r.end, value));
        }

        // For each affected cell, find the text span and apply replacements.
        let mut edits: Vec<Span<TextLocation, TextData>> = Vec::new();
        for ((row_idx, col_idx), replacements) in &mut by_cell {
            // Map row_index -> line_number -> span indices.
            let line_num = line_numbers.get(*row_idx).ok_or_else(|| {
                Error::validation(
                    format!(
                        "row_index {} out of bounds (have {} rows)",
                        row_idx,
                        line_numbers.len()
                    ),
                    "tabular-redact",
                )
            })?;
            let row_spans = rows.get(line_num).ok_or_else(|| {
                Error::validation(
                    format!("no spans for line_number {line_num}"),
                    "tabular-redact",
                )
            })?;
            let &span_idx = row_spans.get(*col_idx).ok_or_else(|| {
                Error::validation(
                    format!(
                        "column_index {} out of bounds in row {} (have {} columns)",
                        col_idx,
                        row_idx,
                        row_spans.len()
                    ),
                    "tabular-redact",
                )
            })?;

            let span = &all_spans[span_idx];
            let content: &str = span.data.as_ref();

            // Sort right-to-left so earlier byte offsets stay valid.
            replacements.sort_by(|a, b| b.0.cmp(&a.0));

            // Check for overlapping ranges.
            for pair in replacements.windows(2) {
                let (later_start, _, _) = &pair[0];
                let (earlier_start, earlier_end, _) = &pair[1];
                if *earlier_end > *later_start {
                    return Err(Error::validation(
                        format!(
                            "overlapping redaction ranges: {}..{} and {}..{}",
                            earlier_start, earlier_end, later_start, pair[0].1,
                        ),
                        "tabular-redact",
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
                        "tabular-redact",
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

        tracing::debug!(target: TARGET, edit_count, "tabular redactions applied");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::Result;
    use nvisy_ontology::entity::TabularLocation;

    use super::*;
    use crate::handler::{CsvData, CsvHandler};
    use crate::transform::TextOutput;

    fn handler() -> CsvHandler {
        CsvHandler::new(CsvData {
            headers: Some(vec!["name".into(), "ssn".into()]),
            rows: vec![
                vec!["Alice Smith".into(), "123-45-6789".into()],
                vec!["Bob Jones".into(), "987-65-4321".into()],
            ],
            delimiter: b',',
            trailing_newline: true,
        })
    }

    fn redaction(
        row: usize,
        col: usize,
        start: usize,
        end: usize,
        replacement: &str,
    ) -> TabularRedaction {
        TabularRedaction {
            location: TabularLocation {
                value: String::new(),
                row_index: row,
                column_index: col,
                start_offset: None,
                end_offset: None,
                column_name: None,
                sheet_name: None,
            },
            start,
            end,
            output: TextOutput::replace(replacement),
        }
    }

    #[tokio::test]
    async fn single_cell_redaction() -> Result<()> {
        let mut h = handler();
        // row 1 = first data row (headers are row 0), col 1 = ssn
        let r = redaction(1, 1, 0, 11, "[REDACTED]");
        TabularTransform::redact_tabular(&mut h, &[r]).await?;
        assert_eq!(h.cell(0, 1), Some("[REDACTED]"));
        Ok(())
    }

    #[tokio::test]
    async fn partial_cell_redaction() -> Result<()> {
        let mut h = handler();
        // Redact "Alice" (0..5) in the name cell at row 1, col 0.
        let r = redaction(1, 0, 0, 5, "[NAME]");
        TabularTransform::redact_tabular(&mut h, &[r]).await?;
        assert_eq!(h.cell(0, 0), Some("[NAME] Smith"));
        Ok(())
    }

    #[tokio::test]
    async fn multiple_cells_redacted() -> Result<()> {
        let mut h = handler();
        let r1 = redaction(1, 1, 0, 11, "[REDACTED]");
        let r2 = redaction(2, 1, 0, 11, "[REDACTED]");
        TabularTransform::redact_tabular(&mut h, &[r1, r2]).await?;
        assert_eq!(h.cell(0, 1), Some("[REDACTED]"));
        assert_eq!(h.cell(1, 1), Some("[REDACTED]"));
        Ok(())
    }

    #[tokio::test]
    async fn empty_redactions_is_noop() -> Result<()> {
        let mut h = handler();
        TabularTransform::redact_tabular(&mut h, &[]).await?;
        assert_eq!(h.cell(0, 0), Some("Alice Smith"));
        Ok(())
    }

    #[tokio::test]
    async fn remove_cell_content() -> Result<()> {
        let mut h = handler();
        let r = TabularRedaction {
            location: TabularLocation {
                value: String::new(),
                row_index: 1,
                column_index: 1,
                start_offset: None,
                end_offset: None,
                column_name: None,
                sheet_name: None,
            },
            start: 0,
            end: 11,
            output: TextOutput::Remove,
        };
        TabularTransform::redact_tabular(&mut h, &[r]).await?;
        assert_eq!(h.cell(0, 1), Some(""));
        Ok(())
    }

    #[tokio::test]
    async fn header_redaction() -> Result<()> {
        let mut h = handler();
        // Row 0 = headers
        let r = redaction(0, 1, 0, 3, "[REDACTED]");
        TabularTransform::redact_tabular(&mut h, &[r]).await?;
        assert_eq!(
            h.headers(),
            Some(["name".to_string(), "[REDACTED]".to_string()].as_slice())
        );
        Ok(())
    }

    #[tokio::test]
    async fn row_out_of_bounds() {
        let mut h = handler();
        let r = redaction(99, 0, 0, 1, "x");
        let err = TabularTransform::redact_tabular(&mut h, &[r])
            .await
            .unwrap_err();
        assert!(err.to_string().contains("row_index 99 out of bounds"));
    }

    #[tokio::test]
    async fn col_out_of_bounds() {
        let mut h = handler();
        let r = redaction(0, 99, 0, 1, "x");
        let err = TabularTransform::redact_tabular(&mut h, &[r])
            .await
            .unwrap_err();
        assert!(err.to_string().contains("column_index 99 out of bounds"));
    }
}

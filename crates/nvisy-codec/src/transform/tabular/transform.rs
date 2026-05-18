//! [`TabularTransform`] async trait and blanket implementation.
//!
//! Bridges [`TabularRedaction`] (cell-addressed by row/col) to the
//! underlying [`TextHandler`] (byte-offset-addressed spans).
//!
//! The blanket implementation walks the per-cell groups in a
//! [`Redactions`] collection, locates each cell's text span via
//! `line_number`/column-position, applies intra-cell byte-offset
//! replacements right-to-left, and writes results back via
//! [`TextHandler::edit_text`].
//!
//! Overlap detection is owned by [`Redactions`]; this transform
//! trusts that ranges within a single cell do not overlap.

use std::cmp::Reverse;
use std::collections::HashMap;

use futures::StreamExt;
use nvisy_core::Error;
use nvisy_ontology::entity::{TabularLocation, TextLocation};

use super::instruction::TabularRedaction;
use crate::document::{Span, SpanStream};
use crate::handler::{TextData, TextHandler};
use crate::transform::Redactions;

const TARGET: &str = "nvisy_codec::transform::tabular";

/// Extension trait for text handlers that support cell-addressed
/// tabular redaction.
///
/// Implemented automatically for all [`TextHandler`] types via a
/// blanket impl. The trait translates `(row, col)` cell addresses
/// into the byte-offset [`TextLocation`]s that the handler understands.
#[async_trait::async_trait]
pub trait TabularTransform: TextHandler {
    /// Apply a batch of cell-addressed redactions, mutating in place.
    ///
    /// Redactions are grouped by [`TabularLocation`] cell in the
    /// input [`Redactions`] collection. Overlap detection per cell is
    /// handled by the collection on insert.
    async fn redact_tabular(
        &mut self,
        redactions: Redactions<TabularLocation, TabularRedaction>,
    ) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl<H: TextHandler> TabularTransform for H {
    async fn redact_tabular(
        &mut self,
        redactions: Redactions<TabularLocation, TabularRedaction>,
    ) -> Result<(), Error> {
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

        let mut edits: Vec<Span<TextLocation, TextData>> = Vec::new();
        for (cell, mut items) in redactions {
            // Map row_index -> line_number -> span indices.
            let line_num = line_numbers.get(cell.row_index).ok_or_else(|| {
                Error::validation(
                    format!(
                        "row_index {} out of bounds (have {} rows)",
                        cell.row_index,
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
            let &span_idx = row_spans.get(cell.column_index).ok_or_else(|| {
                Error::validation(
                    format!(
                        "column_index {} out of bounds in row {} (have {} columns)",
                        cell.column_index,
                        cell.row_index,
                        row_spans.len()
                    ),
                    "tabular-redact",
                )
            })?;

            let span = &all_spans[span_idx];
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
    use crate::transform::{ConflictPolicy, TextOutput};

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

    fn cell(row: usize, col: usize) -> TabularLocation {
        TabularLocation::builder()
            .with_row_index(row)
            .with_column_index(col)
            .build()
            .expect("required fields provided")
    }

    fn redaction(start: usize, end: usize, replacement: &str) -> TabularRedaction {
        TabularRedaction::new(start, end, TextOutput::replace(replacement))
    }

    #[tokio::test]
    async fn single_cell_redaction() -> Result<()> {
        let mut h = handler();
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        // row 1 = first data row (headers are row 0), col 1 = ssn
        rs.try_insert(cell(1, 1), redaction(0, 11, "[REDACTED]"))
            .unwrap();
        TabularTransform::redact_tabular(&mut h, rs).await?;
        assert_eq!(h.cell(0, 1), Some("[REDACTED]"));
        Ok(())
    }

    #[tokio::test]
    async fn partial_cell_redaction() -> Result<()> {
        let mut h = handler();
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        // Redact "Alice" (0..5) in the name cell at row 1, col 0.
        rs.try_insert(cell(1, 0), redaction(0, 5, "[NAME]"))
            .unwrap();
        TabularTransform::redact_tabular(&mut h, rs).await?;
        assert_eq!(h.cell(0, 0), Some("[NAME] Smith"));
        Ok(())
    }

    #[tokio::test]
    async fn multiple_cells_redacted() -> Result<()> {
        let mut h = handler();
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(cell(1, 1), redaction(0, 11, "[REDACTED]"))
            .unwrap();
        rs.try_insert(cell(2, 1), redaction(0, 11, "[REDACTED]"))
            .unwrap();
        TabularTransform::redact_tabular(&mut h, rs).await?;
        assert_eq!(h.cell(0, 1), Some("[REDACTED]"));
        assert_eq!(h.cell(1, 1), Some("[REDACTED]"));
        Ok(())
    }

    #[tokio::test]
    async fn empty_redactions_is_noop() -> Result<()> {
        let mut h = handler();
        let rs: Redactions<TabularLocation, TabularRedaction> = Redactions::default();
        TabularTransform::redact_tabular(&mut h, rs).await?;
        assert_eq!(h.cell(0, 0), Some("Alice Smith"));
        Ok(())
    }

    #[tokio::test]
    async fn remove_cell_content() -> Result<()> {
        let mut h = handler();
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(cell(1, 1), TabularRedaction::new(0, 11, TextOutput::Remove))
            .unwrap();
        TabularTransform::redact_tabular(&mut h, rs).await?;
        assert_eq!(h.cell(0, 1), Some(""));
        Ok(())
    }

    #[tokio::test]
    async fn header_redaction() -> Result<()> {
        let mut h = handler();
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        // Row 0 = headers
        rs.try_insert(cell(0, 1), redaction(0, 3, "[REDACTED]"))
            .unwrap();
        TabularTransform::redact_tabular(&mut h, rs).await?;
        assert_eq!(
            h.headers(),
            Some(["name".to_string(), "[REDACTED]".to_string()].as_slice())
        );
        Ok(())
    }

    #[tokio::test]
    async fn row_out_of_bounds() {
        let mut h = handler();
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(cell(99, 0), redaction(0, 1, "x")).unwrap();
        let err = TabularTransform::redact_tabular(&mut h, rs)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("row_index 99 out of bounds"));
    }

    #[tokio::test]
    async fn col_out_of_bounds() {
        let mut h = handler();
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(cell(0, 99), redaction(0, 1, "x")).unwrap();
        let err = TabularTransform::redact_tabular(&mut h, rs)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("column_index 99 out of bounds"));
    }
}

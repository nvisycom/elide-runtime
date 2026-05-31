//! Native tabular extraction: walk the codec handle and produce
//! one [`Block<Tabular>`] per row, with per-cell source-mapped
//! spans.
//!
//! The codec emits one `Located<Tabular>` per cell. This module
//! groups cells by row, concatenates their values into the row's
//! flat text (tab-separated), and emits one [`Block<Tabular>`] per
//! row whose [`Span<Tabular>`]s map each cell substring back to its
//! `(row, column)` coordinate.
//!
//! [`Block<Tabular>`]: nvisy_ontology::document::Block
//! [`Span<Tabular>`]: nvisy_ontology::document::Span

use std::collections::BTreeMap;

use futures::StreamExt;
use nvisy_core::Result;
use nvisy_ontology::document::{Block, Span};
use nvisy_ontology::modality::{Tabular, TabularBlock};

use super::{ExtractDispatch, Extraction, ExtractionEngine, PlanSlice, TabularPlan};
use crate::pipeline::PhaseTarget;

const TARGET: &str = "nvisy_engine::extraction::tabular";

/// Cell separator used to concatenate per-row text. Tab is chosen
/// because cell values rarely contain it, so the resulting flat
/// text round-trips back to per-cell ranges without ambiguity.
const CELL_SEPARATOR: &str = "\t";

/// Append one [`Block<Tabular>`] per row to the target's document.
/// Each block carries the concatenated row text and one span per
/// cell mapping the cell's substring range back to the codec's
/// per-cell [`Tabular`] coordinates.
async fn populate_document(target: &mut PhaseTarget<'_, Tabular>) {
    let locations: Vec<_> = {
        let guard = target.handle.lock().await;
        guard.tabular_locations().collect().await
    };
    if locations.is_empty() {
        return;
    }

    // Group cells by row, preserving column order within each row.
    let mut rows: BTreeMap<u32, Vec<Tabular>> = BTreeMap::new();
    for located in locations {
        rows.entry(located.location.row_index)
            .or_default()
            .push(located.location);
    }
    for cells in rows.values_mut() {
        cells.sort_by_key(|c| c.column_index);
    }

    let mut blocks = Vec::with_capacity(rows.len());
    // Iterate by row in BTreeMap key order (ascending row_index).
    for cells in rows.into_values() {
        let mut text = String::new();
        let mut spans = Vec::with_capacity(cells.len());
        for (i, cell) in cells.into_iter().enumerate() {
            if i > 0 {
                text.push_str(CELL_SEPARATOR);
            }
            let Some(value) = target.handle.lock().await.read_tabular(&cell).await else {
                continue;
            };
            let value = value.into_inner();
            let start = text.len();
            text.push_str(&value);
            let end = text.len();
            spans.push(Span {
                text_start: start,
                text_end: end,
                confidence: None,
                source: cell,
            });
        }
        let block = Block::new(TabularBlock::Row { text }).with_spans(spans);
        blocks.push(block);
    }

    tracing::debug!(
        target: TARGET,
        rows = blocks.len(),
        "populated tabular document",
    );

    target.doc.blocks.extend(blocks);
}

#[async_trait::async_trait]
impl ExtractDispatch<Tabular> for ExtractionEngine {
    type Plan = TabularPlan;

    async fn extract(
        &self,
        target: &mut PhaseTarget<'_, Tabular>,
        _plan: &TabularPlan,
    ) -> Result<()> {
        populate_document(target).await;
        Ok(())
    }
}

impl PlanSlice<Tabular> for Extraction {
    fn slice(&self) -> &TabularPlan {
        &self.tabular
    }
}

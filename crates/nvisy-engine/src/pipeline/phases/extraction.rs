//! [`ExtractionPhase`]: Document-walking glue around
//! [`ExtractionEngine`].
//!
//! Walks the codec handle, populates each document's `blocks`. Holds
//! an [`ExtractionEngine`] by value — the engine's two
//! `Option<Arc<…>>` fields keep the underlying OCR/STT services
//! shared across runs without an outer `Arc` wrap.
//!
//! [`ExtractionEngine`]: crate::extraction::ExtractionEngine

use nvisy_core::Result;
use tracing::Instrument;

use crate::core::{DocumentTree, RunContext};
use crate::extraction::ExtractionEngine;
use crate::pipeline::EngineInput;

const TARGET: &str = "nvisy_engine::extraction";

/// Extraction phase orchestrator.
pub struct ExtractionPhase {
    engine: ExtractionEngine,
}

impl ExtractionPhase {
    /// Build the phase from the shared extraction engine. Called
    /// once per pipeline by the pipeline orchestrator.
    pub fn new(engine: ExtractionEngine) -> Self {
        Self { engine }
    }

    /// Walk the tree and run the per-modality extractor against each
    /// node. Visits the root first, then iterates nested embedded
    /// documents; each per-node body borrows the engine, handle, and
    /// metadata directly from this scope.
    pub(crate) async fn apply(
        &self,
        _ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "extraction");
        // Snapshot the tree-owned fields so they don't conflict with
        // the per-node `&mut` borrows produced by `root_mut` /
        // `embeds_mut` further down.
        let handle = tree.handle.clone();
        let metadata = tree.metadata.clone();
        async move {
            self.engine
                .dispatch(tree.root_mut(), &handle, &metadata, &input.plan.extraction)
                .await?;
            for node in tree.embeds_mut() {
                self.engine
                    .dispatch(node, &handle, &metadata, &input.plan.extraction)
                    .await?;
            }
            Ok(())
        }
        .instrument(span)
        .await
    }
}

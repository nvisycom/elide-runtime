//! Detection phase: recognizer registry + per-modality dispatch.
//!
//! Public surface is [`DetectionPhase`] plus the
//! [`RecognizerRegistry`] re-exported from this module.
//!
//! Pattern and NER are the active recognizers. LLM and VLM modules
//! exist on disk (`detection/llm/`, `detection/vlm/`) but are not
//! compiled into the binary — they predate the trait collapse to
//! [`nvisy_core::Recognizer<M>`] and need a rework to reimplement
//! that trait directly. Until then they're parked.

mod engine;
mod lift;
mod ner;
mod pattern;

use nvisy_core::Result;
use tracing::Instrument;

pub use self::engine::RecognizerRegistry;
pub use self::lift::{LiftFromBlock, ProjectIntoBlock};
use crate::core::{DocumentTree, RunContext};
use crate::pipeline::EngineInput;

const TARGET: &str = "nvisy_engine::detection";

/// Detection phase: runs every registered recognizer over each
/// node's blocks and writes [`EntityRecord`]s to `doc.audit`.
///
/// Holds a [`RecognizerRegistry`] by value — the registry's
/// recognizer lists keep the underlying recognizers shared via `Arc`
/// inside, without an outer wrap.
///
/// [`EntityRecord`]: nvisy_ontology::provenance::EntityRecord
pub struct DetectionPhase {
    registry: RecognizerRegistry,
}

impl DetectionPhase {
    /// Build the phase from the shared recognizer registry. Called
    /// once per pipeline by [`DocumentPipeline::from_context`].
    ///
    /// [`DocumentPipeline::from_context`]: crate::pipeline::DocumentPipeline::from_context
    pub fn new(registry: RecognizerRegistry) -> Self {
        Self { registry }
    }

    /// Walk the tree and dispatch the right recognizers per modality
    /// node. Visits the root first, then iterates nested embedded
    /// documents; each per-node body borrows the registry, handle,
    /// and detection plan directly from this scope.
    pub(crate) async fn apply(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "detection");
        let run_id = ctx.shared().run_id;
        // Snapshot the tree-owned handle so it doesn't conflict with
        // the per-node `&mut` borrows produced by `root_mut` /
        // `embeds_mut` further down.
        let handle = tree.handle.clone();
        async move {
            self.registry
                .dispatch(tree.root_mut(), &handle, &input.plan.detection, run_id)
                .await?;
            for node in tree.embeds_mut() {
                self.registry
                    .dispatch(node, &handle, &input.plan.detection, run_id)
                    .await?;
            }
            Ok(())
        }
        .instrument(span)
        .await
    }
}

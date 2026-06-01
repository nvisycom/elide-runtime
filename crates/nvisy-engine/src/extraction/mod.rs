//! Extraction phase: per-modality extractors + shared registry.
//!
//! Public surface is [`ExtractionPhase`] (plus the engine + config
//! re-exported from the `engine` submodule). The phase owns an
//! `Arc<ExtractionEngine>` and dispatches per-node by matching on
//! [`NodeMut`] inside its `apply` method.
//!
//! Per-modality behaviour:
//!
//! - `text` / `tabular` — codec-native; no backend call.
//! - `image` — OCR (when `image` feature is on).
//! - `audio` — STT (when `audio` feature is on).

mod audio;
mod config;
mod engine;
mod image;
mod plan;
mod tabular;
mod text;

use std::sync::Arc;

use nvisy_core::Result;
use nvisy_core::content::ContentMetadata;
use nvisy_ontology::document::Document;
use nvisy_ontology::modality::{Audio, Image};
use tracing::Instrument;

#[cfg(feature = "audio")]
pub use self::audio::{SttExtractor, SttExtractorConfig};
pub use self::config::ExtractionConfig;
pub use self::engine::ExtractionEngine;
#[cfg(feature = "image")]
pub use self::image::{OcrExtractor, OcrExtractorConfig};
pub use self::plan::{AudioPlan, Extraction, ImagePlan, TabularPlan, TextPlan};
use crate::core::{DocumentTree, NodeMut, RunContext, SharedHandle};
use crate::pipeline::EngineInput;

const TARGET: &str = "nvisy_engine::extraction";

/// Extraction phase: walks the codec handle, populates each
/// document's `blocks`. Holds an `Arc<ExtractionEngine>` shared
/// across every run.
pub struct ExtractionPhase {
    engine: Arc<ExtractionEngine>,
}

impl ExtractionPhase {
    /// Build the phase from the shared extraction engine. Called
    /// once per pipeline by [`DocumentPipeline::from_context`].
    ///
    /// [`DocumentPipeline::from_context`]: crate::pipeline::DocumentPipeline::from_context
    pub fn new(engine: Arc<ExtractionEngine>) -> Self {
        Self { engine }
    }

    /// Walk the tree and run the per-modality extractor against each
    /// node.
    pub(crate) async fn apply(
        &self,
        _ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "extraction");
        let handle = tree.handle.clone();
        let metadata = tree.metadata.clone();
        let engine = Arc::clone(&self.engine);
        let plan = Arc::new(input.plan.extraction.clone());
        async move {
            tree.walk_mut(move |node| {
                let engine = Arc::clone(&engine);
                let handle = handle.clone();
                let metadata = metadata.clone();
                let plan = Arc::clone(&plan);
                Box::pin(async move { dispatch(&engine, node, &handle, &metadata, &plan).await })
            })
            .await
        }
        .instrument(span)
        .await
    }
}

async fn dispatch(
    engine: &ExtractionEngine,
    node: NodeMut<'_>,
    handle: &SharedHandle,
    metadata: &ContentMetadata,
    plan: &Extraction,
) -> Result<()> {
    match node {
        NodeMut::Text(doc) => {
            engine.populate_text_blocks(doc, handle).await;
            engine.populate_image_embeds(doc, handle).await;
            let _ = &plan.text;
        }
        NodeMut::Tabular(doc) => {
            self::tabular::populate_document(doc, handle).await;
            let _ = &plan.tabular;
        }
        NodeMut::Image(doc) => dispatch_image(engine, doc, handle, &plan.image).await?,
        NodeMut::Audio(doc) => dispatch_audio(engine, doc, handle, metadata, &plan.audio).await?,
    }
    Ok(())
}

#[cfg(feature = "image")]
async fn dispatch_image(
    engine: &ExtractionEngine,
    doc: &mut Document<Image>,
    handle: &SharedHandle,
    _plan: &ImagePlan,
) -> Result<()> {
    if let Some(ref ocr) = engine.ocr {
        ocr.run_on_doc(doc, handle).await?;
    }
    Ok(())
}

#[cfg(not(feature = "image"))]
async fn dispatch_image(
    _engine: &ExtractionEngine,
    _doc: &mut Document<Image>,
    _handle: &SharedHandle,
    _plan: &ImagePlan,
) -> Result<()> {
    Ok(())
}

#[cfg(feature = "audio")]
async fn dispatch_audio(
    engine: &ExtractionEngine,
    doc: &mut Document<Audio>,
    handle: &SharedHandle,
    metadata: &ContentMetadata,
    plan: &AudioPlan,
) -> Result<()> {
    if let Some(ref stt) = engine.stt {
        stt.run(doc, handle, metadata, plan.diarization).await?;
    }
    Ok(())
}

#[cfg(not(feature = "audio"))]
async fn dispatch_audio(
    _engine: &ExtractionEngine,
    _doc: &mut Document<Audio>,
    _handle: &SharedHandle,
    _metadata: &ContentMetadata,
    _plan: &AudioPlan,
) -> Result<()> {
    Ok(())
}

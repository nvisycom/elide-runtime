//! Extraction: per-modality extractors + shared registry.
//!
//! Each modality lives in its own (private) sub-module — `text`,
//! `tabular`, `image`, `audio` — and owns its `Extract<M>` +
//! `WorkflowSlice<M>` impls. This file holds only the shared
//! scaffolding: the [`Extractors`] registry, its [`ExtractionSection`]
//! config, the [`Extract<M>`] trait, and the [`WorkflowSlice<M>`]
//! helper that lets the orchestrator pull the right slice of
//! [`Extraction`] per modality.
//!
//! Per-modality behaviour:
//!
//! - `text` / `tabular` — codec-native; no backend call.
//! - `image` — OCR (when `image` feature is on).
//! - `audio` — STT (when `audio` feature is on).

mod audio;
mod image;
mod tabular;
mod text;
mod workflow;

use std::sync::Arc;

use nvisy_core::Result;
use nvisy_ontology::modality::Modality;
use serde::{Deserialize, Serialize};

#[cfg(feature = "audio")]
pub use self::audio::{SttExtractor, SttExtractorConfig};
#[cfg(feature = "image")]
pub use self::image::{OcrExtractor, OcrExtractorConfig};
pub use self::workflow::{
    AudialWorkflow, Extraction, ImageWorkflow, TabularWorkflow, TextWorkflow,
};
use crate::envelope::DocumentEnvelope;

/// Registry of pre-built extractors, one per technique.
///
/// Each slot is `Option<Arc<_>>` because the corresponding
/// `[extractor.*]` section is itself optional — operators only
/// configure the techniques they need.
#[derive(Default, Clone)]
pub struct Extractors {
    /// Pre-built OCR extractor (when `[extractor.ocr]` is set).
    #[cfg(feature = "image")]
    pub ocr: Option<Arc<OcrExtractor>>,
    /// Pre-built STT extractor (when `[extractor.stt]` is set).
    #[cfg(feature = "audio")]
    pub stt: Option<Arc<SttExtractor>>,
}

/// Configuration for the [`Extractors`] registry.
///
/// Each field maps to a `[extractor.*]` section in `Nvisy.toml`.
/// `None` opts the technique out entirely.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractionSection {
    /// `[extractor.ocr]` — OCR text extraction from images.
    #[cfg(feature = "image")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr: Option<OcrExtractorConfig>,
    /// `[extractor.stt]` — speech-to-text transcription.
    #[cfg(feature = "audio")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stt: Option<SttExtractorConfig>,
}

impl ExtractionSection {
    /// `true` when every technique is `None`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        #[cfg(feature = "image")]
        let ocr_empty = self.ocr.is_none();
        #[cfg(not(feature = "image"))]
        let ocr_empty = true;
        #[cfg(feature = "audio")]
        let stt_empty = self.stt.is_none();
        #[cfg(not(feature = "audio"))]
        let stt_empty = true;
        ocr_empty && stt_empty
    }
}

impl Extractors {
    /// Build the registry once from an [`ExtractionSection`].
    ///
    /// Each opted-in section drives one extractor construction.
    /// Construction is eager — HTTP clients and OCR/STT engines
    /// build here so per-run dispatch stays cheap.
    ///
    /// # Errors
    ///
    /// Returns the first construction error encountered.
    pub fn from_config(cfg: &ExtractionSection) -> Result<Self> {
        #[cfg(not(any(feature = "image", feature = "audio")))]
        let _ = cfg;
        #[cfg(feature = "image")]
        let ocr = cfg
            .ocr
            .as_ref()
            .filter(|c| c.enabled)
            .map(|c| OcrExtractor::from_config(c.clone()).map(Arc::new))
            .transpose()?;
        #[cfg(feature = "audio")]
        let stt = cfg
            .stt
            .as_ref()
            .filter(|c| c.enabled)
            .map(|c| SttExtractor::from_config(c.clone()).map(Arc::new))
            .transpose()?;
        Ok(Self {
            #[cfg(feature = "image")]
            ocr,
            #[cfg(feature = "audio")]
            stt,
        })
    }

    /// `true` when no extractors are configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        #[cfg(feature = "image")]
        let ocr_empty = self.ocr.is_none();
        #[cfg(not(feature = "image"))]
        let ocr_empty = true;
        #[cfg(feature = "audio")]
        let stt_empty = self.stt.is_none();
        #[cfg(not(feature = "audio"))]
        let stt_empty = true;
        ocr_empty && stt_empty
    }
}

/// Per-modality extraction dispatch.
///
/// The orchestrator calls
/// `Extract::<M>::extract(extractors, envelope, plan.extraction.workflow_for::<M>())`
/// and Rust monomorphizes to the matching per-modality impl in
/// the `text`, `tabular`, `image`, or `audio` sub-module.
///
/// Each impl declares its own [`Workflow`] config struct via the
/// associated type. The orchestrator fishes the right field out of
/// [`Extraction`] via [`Extraction::workflow_for`] so the trait
/// stays narrow.
///
/// [`Workflow`]: Self::Workflow
#[async_trait::async_trait]
pub trait Extract<M: Modality>: Send + Sync {
    /// Per-modality workflow config struct.
    type Workflow: Default + Send + Sync;

    async fn extract(
        &self,
        envelope: &mut DocumentEnvelope<M>,
        workflow: &Self::Workflow,
    ) -> Result<()>;
}

impl Extraction {
    /// Borrow the per-modality workflow slice keyed by `M`.
    ///
    /// Used by the orchestrator to hand `Extract::<M>::extract` the
    /// right slice without each call site having to know the field
    /// name. The bound `Extractors: Extract<M>` ties the return type
    /// to the matching `Workflow` so the call typechecks.
    pub fn workflow_for<M>(&self) -> &<Extractors as Extract<M>>::Workflow
    where
        M: Modality,
        Extractors: Extract<M>,
        Self: WorkflowSlice<M>,
    {
        <Self as WorkflowSlice<M>>::slice(self)
    }
}

/// Helper trait that picks the per-modality workflow field out of
/// [`Extraction`]. One impl per modality, co-located with each
/// modality's [`Extract<M>`] impl in its sub-module.
pub trait WorkflowSlice<M: Modality>
where
    Extractors: Extract<M>,
{
    fn slice(&self) -> &<Extractors as Extract<M>>::Workflow;
}

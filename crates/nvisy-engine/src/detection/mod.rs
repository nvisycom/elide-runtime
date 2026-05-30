//! Detection: [`Recognizer`] trait, [`DetectionEngine`] orchestrator,
//! built-in recognizer adapters (NER, pattern, LLM, VLM).
//!
//! The trait stays in-crate because every implementation today is
//! an engine-side wrapper around a backend (`nvisy-pattern`,
//! `nvisy-ner`, `nvisy-agent`); backend crates themselves stay
//! shape-agnostic.

mod context;
mod dyn_recognizer;
mod lift;
mod llm;
mod ner;
mod pattern;
mod recognizer;
mod recognizers;
mod vlm;

use std::fmt;
use std::sync::Arc;

use derive_builder::Builder;
pub use nvisy_agent::agent::LlmNerContext;
use nvisy_agent::agent::NerHint;
use nvisy_core::Result;
pub use nvisy_ner::Context as NerContext;
use nvisy_ontology::entity::{Annotation, AnnotationKind, AnnotationStrength, Entity};
use nvisy_ontology::modality::{Audio, Image, Modality, ModalityBlock, Overlap, Tabular, Text};
use nvisy_ontology::primitive::ConfidenceThreshold;
pub use nvisy_pattern::{PatternContext, PatternFilter};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;
use tracing::Instrument;
use validator::Validate;

pub use self::context::{
    DetectionContext, DetectionContextBuilder, DetectionContextBuilderError, VlmDetectionContext,
};
pub use self::dyn_recognizer::{DynImageRecognizer, DynTextRecognizer};
pub use self::lift::{LiftFromBlock, ProjectIntoBlock};
pub use self::llm::{
    DetectParams, LlmDetection, VerifyParams, build_pipeline as build_llm_pipeline,
};
pub use self::ner::{NerDetection, NerRecognizer};
pub use self::pattern::{PatternDetection, PatternRecognizer};
pub use self::recognizer::{Recognizer, RecognizerKind};
pub use self::recognizers::{DetectionSection, ImageRecognizers, Recognizers, TextRecognizers};
pub use self::vlm::{
    VlmDetectParams, VlmDetection, VlmVerifyParams, build_pipeline as build_vlm_pipeline,
};
use crate::envelope::DocumentEnvelope;

const TARGET: &str = "nvisy_engine::detection";

/// Composite detection engine.
///
/// Holds two parallel lists of recognizers — one per modality — and
/// dispatches the matching list against a per-modality context.
///
/// Parallelism uses [`JoinSet`]: each recognizer runs on its own
/// task so CPU-bound work and I/O-bound work overlap. Failure is
/// fail-fast within a list: on the first task error every other
/// in-flight task in that list is aborted and the error is returned.
///
/// Per-modality dispatch lives in the [`Detect`] trait impls below
/// — text blocks fan out their `scan_text` to every text recognizer
/// (lifted into document coordinates via the block's spans); image
/// envelopes fan out every image location to every image recognizer
/// (entities are emitted in absolute image coordinates by the
/// recognizer itself, no lifting).
///
/// Dedup, conflict resolution, and threshold filtering are *not*
/// the engine's concern — those live in the downstream pipeline
/// (`nvisy-engine::operation::deduplication`).
///
/// Construct via [`builder`]. At least one recognizer (text or
/// image) must be attached; calling [`build`] without one returns a
/// `Misconfigured` error.
///
/// [`JoinSet`]: tokio::task::JoinSet
/// [`builder`]: Self::builder
/// [`build`]: DetectionEngineBuilder::build
#[derive(Builder)]
#[builder(
    name = "DetectionEngineBuilder",
    pattern = "owned",
    build_fn(error = "DetectionEngineBuilderError", validate = "Self::validate")
)]
pub struct DetectionEngine {
    #[builder(setter(custom), default)]
    text: Vec<Arc<dyn DynTextRecognizer>>,
    #[builder(setter(custom), default)]
    image: Vec<Arc<dyn DynImageRecognizer>>,
}

impl DetectionEngineBuilder {
    /// Attach a text-modality recognizer. May be called multiple
    /// times; recognizers run in the order they were attached.
    pub fn with_text_recognizer<R>(mut self, recognizer: R) -> Self
    where
        R: Recognizer<Modality = Text> + 'static,
        R::Context: for<'a> From<&'a DetectionContext> + Send + Sync,
    {
        self.text
            .get_or_insert_with(Vec::new)
            .push(Arc::new(recognizer));
        self
    }

    /// Attach a text-modality recognizer already wrapped in `Arc`.
    pub fn with_text_recognizer_arc<R>(mut self, recognizer: Arc<R>) -> Self
    where
        R: Recognizer<Modality = Text> + 'static,
        R::Context: for<'a> From<&'a DetectionContext> + Send + Sync,
    {
        let dyn_rec: Arc<dyn DynTextRecognizer> = recognizer;
        self.text.get_or_insert_with(Vec::new).push(dyn_rec);
        self
    }

    /// Attach an image-modality recognizer. May be called multiple
    /// times; recognizers run in the order they were attached.
    pub fn with_image_recognizer<R>(mut self, recognizer: R) -> Self
    where
        R: Recognizer<Modality = Image> + 'static,
        R::Context: for<'a> From<&'a VlmDetectionContext> + Send + Sync,
    {
        self.image
            .get_or_insert_with(Vec::new)
            .push(Arc::new(recognizer));
        self
    }

    /// Attach an image-modality recognizer already wrapped in `Arc`.
    pub fn with_image_recognizer_arc<R>(mut self, recognizer: Arc<R>) -> Self
    where
        R: Recognizer<Modality = Image> + 'static,
        R::Context: for<'a> From<&'a VlmDetectionContext> + Send + Sync,
    {
        let dyn_rec: Arc<dyn DynImageRecognizer> = recognizer;
        self.image.get_or_insert_with(Vec::new).push(dyn_rec);
        self
    }

    fn validate(&self) -> std::result::Result<(), String> {
        let text_empty = self.text.as_ref().is_none_or(|v| v.is_empty());
        let image_empty = self.image.as_ref().is_none_or(|v| v.is_empty());
        if text_empty && image_empty {
            return Err("at least one recognizer (text or image) must be attached".into());
        }
        Ok(())
    }
}

/// Error returned by [`DetectionEngineBuilder::build`] when the
/// engine is misconfigured (currently: no recognizers attached).
#[derive(Debug, thiserror::Error)]
#[error("DetectionEngine build failed: {0}")]
pub struct DetectionEngineBuilderError(String);

impl From<derive_builder::UninitializedFieldError> for DetectionEngineBuilderError {
    fn from(err: derive_builder::UninitializedFieldError) -> Self {
        Self(format!("missing required field `{}`", err.field_name()))
    }
}

impl From<String> for DetectionEngineBuilderError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl DetectionEngine {
    /// Start building an engine.
    pub fn builder() -> DetectionEngineBuilder {
        DetectionEngineBuilder::default()
    }

    /// Borrow the attached text recognizers, in attach order.
    pub fn text_recognizers(&self) -> &[Arc<dyn DynTextRecognizer>] {
        &self.text
    }

    /// Borrow the attached image recognizers, in attach order.
    pub fn image_recognizers(&self) -> &[Arc<dyn DynImageRecognizer>] {
        &self.image
    }

    /// Run every text recognizer against `ctx` in parallel and
    /// return the combined entity set.
    async fn run_text(&self, ctx: DetectionContext) -> Result<Vec<Entity<Text>>> {
        let span = tracing::debug_span!(
            target: TARGET,
            "detect.text",
            recognizers = self.text.len(),
            text_len = ctx.text.len(),
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        );

        let ctx = Arc::new(ctx);
        let recognizers = self.text.clone();

        async move {
            let mut set: JoinSet<Result<Vec<Entity<Text>>>> = JoinSet::new();
            for recognizer in recognizers {
                let ctx = Arc::clone(&ctx);
                set.spawn(async move { recognizer.run(&ctx).await });
            }
            collect_join_set(set).await
        }
        .instrument(span)
        .await
    }

    /// Run every image recognizer against `ctx` in parallel and
    /// return the combined entity set.
    async fn run_image(&self, ctx: VlmDetectionContext) -> Result<Vec<Entity<Image>>> {
        let span = tracing::debug_span!(
            target: TARGET,
            "detect.image",
            recognizers = self.image.len(),
            image_bytes = ctx.image.len(),
            width = ctx.dims.width,
            height = ctx.dims.height,
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        );

        let ctx = Arc::new(ctx);
        let recognizers = self.image.clone();

        async move {
            let mut set: JoinSet<Result<Vec<Entity<Image>>>> = JoinSet::new();
            for recognizer in recognizers {
                let ctx = Arc::clone(&ctx);
                set.spawn(async move { recognizer.run(&ctx).await });
            }
            collect_join_set(set).await
        }
        .instrument(span)
        .await
    }

    /// Reset per-document state on every attached recognizer.
    /// Stateless recognizers do nothing; LLM/VLM recognizers clear
    /// cumulative usage trackers. Call at document boundaries.
    pub async fn reset(&self) {
        for recognizer in &self.text {
            recognizer.reset().await;
        }
        for recognizer in &self.image {
            recognizer.reset().await;
        }
    }
}

async fn collect_join_set<E: Modality>(
    mut set: JoinSet<Result<Vec<Entity<E>>>>,
) -> Result<Vec<Entity<E>>> {
    let mut all = Vec::new();
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok(Ok(entities)) => {
                tracing::debug!(
                    target: TARGET,
                    detected = entities.len(),
                    "recognizer produced entities",
                );
                all.extend(entities);
            }
            Ok(Err(e)) => {
                set.abort_all();
                return Err(e);
            }
            Err(join_err) => {
                set.abort_all();
                return Err(nvisy_core::Error::runtime(
                    format!("recognizer task panicked or was cancelled: {join_err}"),
                    "detection-engine",
                    false,
                ));
            }
        }
    }
    Ok(all)
}

impl fmt::Debug for DetectionEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DetectionEngine")
            .field("text_recognizers", &self.text.len())
            .field("image_recognizers", &self.image.len())
            .finish_non_exhaustive()
    }
}

/// Per-modality detection dispatch.
///
/// The pipeline's stage method (`DocumentPipeline<M>::run_detection`)
/// calls `Detect::<M>::detect(engine, &mut envelope, &plan.detection)`
/// and Rust monomorphizes to the matching impl below.
///
/// Text/Tabular dispatch text recognizers per block (lifted to
/// document coordinates via the block's spans). Image dispatches
/// text recognizers per OCR'd block *and* image recognizers per
/// image location ("runs alongside"). Audio dispatches text
/// recognizers per transcript block.
#[async_trait::async_trait]
pub trait Detect<M: Modality>: Send + Sync {
    async fn detect(&self, envelope: &mut DocumentEnvelope<M>, cfg: &Detection) -> Result<()>;
}

/// Shared text-side block loop. Used by every modality that exposes
/// text via `ModalityBlock::scan_text` (today: every modality).
async fn detect_text_blocks<M>(
    engine: &DetectionEngine,
    envelope: &mut DocumentEnvelope<M>,
    cfg: &Detection,
) -> Result<()>
where
    M: Modality + LiftFromBlock + ProjectIntoBlock + Overlap,
{
    if engine.text.is_empty() || envelope.document.blocks.is_empty() {
        return Ok(());
    }

    let run_id = envelope.shared.run_id;
    let mut lifted: Vec<Entity<M>> = Vec::new();
    let mut scanned_blocks = 0usize;

    let labels: Vec<String> = envelope
        .document
        .labels
        .iter()
        .map(|l| l.label.clone())
        .collect();

    for block in &envelope.document.blocks {
        let Some(text) = block.kind.scan_text() else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        scanned_blocks += 1;

        let hints = collect_hints_for_block::<M>(&envelope.document.annotations, &block.spans);

        let mut ctx = DetectionContext::new(text.to_owned());
        ctx.correlation_id = Some(run_id);
        if !cfg.entity_kinds.is_empty() {
            ctx.entities = Some(cfg.entity_kinds.clone());
        }
        if let Some(threshold) = cfg.confidence_threshold {
            ctx.score_threshold = Some(threshold);
        }
        ctx.hints = hints;
        ctx.labels = labels.clone();

        let detected = engine.run_text(ctx).await?;
        for entity in detected {
            let Some(location) =
                M::lift_from_block(&block.spans, entity.location.start, entity.location.end)
            else {
                tracing::debug!(
                    target: TARGET,
                    kind = %entity.entity_kind,
                    "dropping entity with no overlapping span",
                );
                continue;
            };
            lifted.push(Entity {
                id: entity.id,
                entity_id: entity.entity_id,
                category: entity.category,
                entity_kind: entity.entity_kind,
                recognition_methods: entity.recognition_methods,
                refinement_methods: entity.refinement_methods,
                confidence: entity.confidence,
                location,
                language: entity.language,
            });
        }
    }

    tracing::debug!(
        target: TARGET,
        detected = lifted.len(),
        blocks = scanned_blocks,
        "appending text-detected entities",
    );
    envelope.add_entities(lifted);
    Ok(())
}

#[async_trait::async_trait]
impl Detect<Text> for DetectionEngine {
    async fn detect(&self, envelope: &mut DocumentEnvelope<Text>, cfg: &Detection) -> Result<()> {
        detect_text_blocks(self, envelope, cfg).await?;
        self.reset().await;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Detect<Tabular> for DetectionEngine {
    async fn detect(
        &self,
        envelope: &mut DocumentEnvelope<Tabular>,
        cfg: &Detection,
    ) -> Result<()> {
        detect_text_blocks(self, envelope, cfg).await?;
        self.reset().await;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Detect<Audio> for DetectionEngine {
    async fn detect(&self, envelope: &mut DocumentEnvelope<Audio>, cfg: &Detection) -> Result<()> {
        detect_text_blocks(self, envelope, cfg).await?;
        self.reset().await;
        Ok(())
    }
}

#[cfg(feature = "image")]
#[async_trait::async_trait]
impl Detect<Image> for DetectionEngine {
    async fn detect(&self, envelope: &mut DocumentEnvelope<Image>, cfg: &Detection) -> Result<()> {
        // 1. Text recognizers run on every OCR'd block ("runs
        //    alongside" image-side recognizers).
        detect_text_blocks(self, envelope, cfg).await?;

        // 2. Image recognizers run once per image location with
        //    the raw image bytes; they emit absolute image-coord
        //    Entity<Image> directly, no per-block lifting.
        if !self.image.is_empty() {
            let run_id = envelope.shared.run_id;
            let labels: Vec<String> = envelope
                .document
                .labels
                .iter()
                .map(|l| l.label.clone())
                .collect();

            let locations = envelope.collect_image_locations().await;
            let mut detected_total = 0usize;
            for located in locations {
                let Some(image_data) = envelope.read_image(&located.location).await else {
                    continue;
                };
                let dims = image_data.dimensions();
                let bytes = image_data.encode_png().map_err(|e| {
                    nvisy_core::Error::runtime(e.to_string(), "detection-engine", false)
                })?;

                let mut ctx = VlmDetectionContext::new(bytes, dims);
                ctx.correlation_id = Some(run_id);
                if !cfg.entity_kinds.is_empty() {
                    ctx.entities = Some(cfg.entity_kinds.clone());
                }
                if let Some(threshold) = cfg.confidence_threshold {
                    ctx.score_threshold = Some(threshold);
                }
                ctx.labels = labels.clone();

                let detected = self.run_image(ctx).await?;
                detected_total += detected.len();
                envelope.add_entities(detected);
            }

            tracing::debug!(
                target: TARGET,
                detected = detected_total,
                "appending image-detected entities",
            );
        }

        self.reset().await;
        Ok(())
    }
}

#[cfg(not(feature = "image"))]
#[async_trait::async_trait]
impl Detect<Image> for DetectionEngine {
    async fn detect(&self, envelope: &mut DocumentEnvelope<Image>, cfg: &Detection) -> Result<()> {
        detect_text_blocks(self, envelope, cfg).await?;
        self.reset().await;
        Ok(())
    }
}

/// Collect [`NerHint`]s for a single block: walk every
/// [`Hint`]-strength [`Inclusion`] annotation, project each one
/// onto the block's text-byte coordinates via [`ProjectIntoBlock`],
/// and emit a hint when the projection succeeds.
///
/// Annotations whose target doesn't overlap any of this block's
/// spans are skipped — they belong to a different block (or to no
/// block, if the user marked a region we never extracted text
/// from). Empty projected ranges are also skipped.
///
/// Exclusions are always assertions (the type system forbids
/// `Hint` exclusions), so this helper only returns inclusion
/// hints. Exclusion enforcement is the post-detection filter's
/// job, not the prompt's.
///
/// [`Hint`]: AnnotationStrength::Hint
/// [`Inclusion`]: AnnotationKind::Inclusion
fn collect_hints_for_block<M>(
    annotations: &[Annotation<M>],
    spans: &[nvisy_ontology::document::Span<M>],
) -> Vec<NerHint>
where
    M: Modality + Overlap + ProjectIntoBlock,
{
    annotations
        .iter()
        .filter_map(|ann| {
            let AnnotationKind::Inclusion {
                category,
                entity_kind,
                target,
                strength: AnnotationStrength::Hint { .. },
            } = &ann.kind
            else {
                return None;
            };
            let (start, end) = M::project_into_block(spans, target)?;
            if start >= end {
                return None;
            }
            Some(NerHint {
                name: ann.name.clone(),
                category: *category,
                entity_kind: *entity_kind,
                start,
                end,
            })
        })
        .collect()
}

/// Workflow detection node — which recognizers to dispatch and the
/// shared per-call hints.
///
/// Recognizer construction lives in [`Recognizers`], built once at
/// engine startup from `[detection.*]` config sections. This node
/// only references already-built recognizers by [`RecognizerKind`].
///
/// [`kinds`] is the enable/disable list — empty means no detection
/// runs for this workflow.
///
/// [`entity_kinds`] and [`confidence_threshold`] are the per-call
/// hints honored by every enabled recognizer. Recognizer-specific
/// build config (provider, model, regex set, etc.) lives in
/// `[detection.*]` runtime config, never here.
///
/// [`kinds`]: Self::kinds
/// [`entity_kinds`]: Self::entity_kinds
/// [`confidence_threshold`]: Self::confidence_threshold
#[derive(Debug, Clone, Default, PartialEq, validator::Validate)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Detection {
    /// Which recognizer kinds to enable for this workflow. Empty
    /// disables detection entirely.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<RecognizerKind>,
    /// Entity-kind allowlist applied to every enabled recognizer.
    /// Empty = all kinds permitted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_kinds: Vec<nvisy_ontology::entity::EntityKind>,
    /// Minimum confidence threshold honored by every recognizer.
    /// `None` disables confidence filtering. The newtype enforces
    /// `[0.0, 1.0]` + finite-float at deserialize, so no `validate`
    /// attribute is needed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_threshold: Option<ConfidenceThreshold>,
}

impl Detection {
    /// Validate the configuration.
    pub fn validate(&self) -> std::result::Result<(), validator::ValidationErrors> {
        Validate::validate(self)
    }

    /// Assemble a [`DetectionEngine`] by picking each enabled kind
    /// from the pre-built [`Recognizers`] registry.
    ///
    /// # Errors
    ///
    /// Returns a validation error if [`kinds`] names a recognizer
    /// whose `[detection.*]` section is not configured, or if
    /// [`kinds`] is empty (no recognizers to dispatch).
    ///
    /// [`kinds`]: Self::kinds
    pub fn into_engine(&self, recognizers: &Recognizers) -> Result<DetectionEngine> {
        let mut builder = DetectionEngine::builder();
        for &kind in &self.kinds {
            recognizers.require(kind)?;
            builder = match kind {
                RecognizerKind::Llm => builder
                    .with_text_recognizer_arc(Arc::clone(recognizers.text.llm.as_ref().unwrap())),
                RecognizerKind::Ner => builder
                    .with_text_recognizer_arc(Arc::clone(recognizers.text.ner.as_ref().unwrap())),
                RecognizerKind::Pattern => builder.with_text_recognizer_arc(Arc::clone(
                    recognizers.text.pattern.as_ref().unwrap(),
                )),
                RecognizerKind::Vlm => builder
                    .with_image_recognizer_arc(Arc::clone(recognizers.image.vlm.as_ref().unwrap())),
            };
        }
        builder
            .build()
            .map_err(|e| nvisy_core::Error::validation(e.to_string(), "detection-engine"))
    }
}

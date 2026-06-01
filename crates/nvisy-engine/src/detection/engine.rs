//! [`DetectionEngine`]: composite per-run recognizer registry.
//!
//! Holds two parallel lists of recognizers — one per modality — and
//! exposes per-modality detection bodies the [`DetectionPhase`]
//! dispatches into from its `apply` walk. Construction is one-shot
//! via [`DetectionEngineBuilder`]; the engine is then shared (`Arc`)
//! by [`DetectionPhase`] across the per-run pipeline.
//!
//! Private helpers ([`detect_text_blocks`], [`collect_hints_for_block`],
//! [`collect_join_set`]) live here too — they're implementation
//! detail of the dispatch path and not re-exported.
//!
//! [`DetectionPhase`]: super::DetectionPhase

use std::fmt;
use std::sync::Arc;

use derive_builder::Builder;
use nvisy_agent::agent::NerHint;
use nvisy_core::Result;
use nvisy_ontology::entity::{Annotation, AnnotationKind, AnnotationStrength, Entity};
use nvisy_ontology::modality::{Image, Modality, ModalityBlock, Overlap, Text};
use tokio::task::JoinSet;
use tracing::Instrument;

use super::Detection;
use super::context::{DetectionContext, VlmDetectionContext};
use super::dyn_recognizer::{DynImageRecognizer, DynTextRecognizer};
use super::lift::{LiftFromBlock, ProjectIntoBlock};
use super::recognizer::Recognizer;

pub(super) const TARGET: &str = "nvisy_engine::detection";

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
/// Per-modality dispatch lives in the [`DetectDispatch`] trait
/// impls below — text blocks fan out their `scan_text` to every text
/// recognizer (lifted into document coordinates via the block's
/// spans); image docs (standalone or nested under a
/// [`TextBlock::Embed`]) fan out every image location to every image
/// recognizer (entities are emitted in absolute image coordinates by
/// the recognizer itself, no lifting). Nested image docs are
/// reached by walking the outer text envelope's blocks; the codec
/// handle that services their reads lives on the outer envelope.
///
/// [`TextBlock::Embed`]: nvisy_ontology::modality::TextBlock::Embed
///
/// Dedup, conflict resolution, and threshold filtering are *not*
/// the engine's concern — those live in the downstream pipeline
/// (`nvisy-engine::operation::deduplication`).
///
/// Construct via [`builder`]. Both recognizer lists may be empty
/// — the engine becomes a no-op pass-through, useful for
/// redaction-only runs or dry runs that skip detection.
///
/// [`JoinSet`]: tokio::task::JoinSet
/// [`builder`]: Self::builder
#[derive(Builder, Clone)]
#[builder(
    name = "DetectionEngineBuilder",
    pattern = "owned",
    build_fn(error = "DetectionEngineBuilderError")
)]
pub struct DetectionEngine {
    #[builder(setter(custom), default)]
    text: Vec<Arc<dyn DynTextRecognizer>>,
    #[builder(setter(custom), default)]
    image: Vec<Arc<dyn DynImageRecognizer>>,
}

impl DetectionEngineBuilder {
    /// Attach a text-modality recognizer already wrapped in `Arc`.
    /// May be called multiple times; recognizers run in the order
    /// they were attached.
    pub fn with_text_recognizer_arc<R>(mut self, recognizer: Arc<R>) -> Self
    where
        R: Recognizer<Modality = Text> + 'static,
        R::Context: for<'a> From<&'a DetectionContext> + Send + Sync,
    {
        let dyn_rec: Arc<dyn DynTextRecognizer> = recognizer;
        self.text.get_or_insert_with(Vec::new).push(dyn_rec);
        self
    }

    /// Attach an image-modality recognizer already wrapped in `Arc`.
    /// May be called multiple times; recognizers run in the order
    /// they were attached.
    pub fn with_image_recognizer_arc<R>(mut self, recognizer: Arc<R>) -> Self
    where
        R: Recognizer<Modality = Image> + 'static,
        R::Context: for<'a> From<&'a VlmDetectionContext> + Send + Sync,
    {
        let dyn_rec: Arc<dyn DynImageRecognizer> = recognizer;
        self.image.get_or_insert_with(Vec::new).push(dyn_rec);
        self
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

/// Shared text-side block loop. Used by every modality that exposes
/// text via `ModalityBlock::scan_text` (today: every modality).
///
/// Recursion into [`TextBlock::Embed`] children is the
/// orchestrator's job; this loop scans the target's *own* blocks
/// only and skips embeds via `scan_text` returning `None`.
///
/// [`TextBlock::Embed`]: nvisy_ontology::modality::TextBlock::Embed
pub(crate) async fn detect_text_blocks<M>(
    engine: &DetectionEngine,
    doc: &mut nvisy_ontology::document::Document<M>,
    cfg: &Detection,
    run_id: uuid::Uuid,
) -> Result<()>
where
    M: Modality + LiftFromBlock + ProjectIntoBlock + Overlap,
{
    if engine.text.is_empty() || doc.blocks.is_empty() {
        return Ok(());
    }

    let mut lifted: Vec<Entity<M>> = Vec::new();
    let mut scanned_blocks = 0usize;

    let labels: Vec<String> = doc.labels.iter().map(|l| l.label.clone()).collect();

    for block in &doc.blocks {
        let Some(text) = block.kind.scan_text() else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        scanned_blocks += 1;

        let hints = collect_hints_for_block::<M>(&doc.annotations, &block.spans);

        let mut ctx = DetectionContext::new(text.to_owned());
        ctx.correlation_id = Some(run_id);
        if !cfg.entity_kinds.is_empty() {
            ctx.entities = Some(cfg.entity_kinds.clone());
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
    doc.add_entities(lifted);
    Ok(())
}

impl DetectionEngine {
    /// Run text recognizers over every block in `doc` and reset
    /// per-document state. Shared by every modality whose detection
    /// is purely text-based.
    pub(crate) async fn detect_text_only<M>(
        &self,
        doc: &mut nvisy_ontology::document::Document<M>,
        cfg: &Detection,
        run_id: uuid::Uuid,
    ) -> Result<()>
    where
        M: Modality + LiftFromBlock + ProjectIntoBlock + Overlap,
    {
        detect_text_blocks(self, doc, cfg, run_id).await?;
        self.reset().await;
        Ok(())
    }

    /// Run image recognizers once per image location reachable via
    /// the target's handle, appending detections to
    /// `target.doc.audit`. Each call sees the raw encoded image
    /// bytes plus the image's pixel dimensions; recognizers emit
    /// absolute image-coord entities directly (no per-block lifting).
    ///
    /// Works for both standalone image envelopes and nested image
    /// docs (a `Document<Image>` inside a `TextBlock::Embed`): in
    /// the nested case the orchestrator builds a `PhaseTarget`
    /// borrowing the outer envelope's handle, so this body never
    /// has to know whether it's looking at root or nested.
    #[cfg(feature = "image")]
    pub(crate) async fn detect_image_locations(
        &self,
        doc: &mut nvisy_ontology::document::Document<Image>,
        handle: &crate::core::SharedHandle,
        cfg: &Detection,
        run_id: uuid::Uuid,
    ) -> Result<()> {
        if self.image.is_empty() {
            return Ok(());
        }

        let labels: Vec<String> = doc.labels.iter().map(|l| l.label.clone()).collect();
        let locations: Vec<_> = {
            use futures::StreamExt;
            handle.lock().await.image_locations().collect().await
        };
        let mut detected_total = 0usize;
        for located in locations {
            let Some(image_data) = handle.lock().await.read_image(&located.location).await else {
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
            ctx.labels = labels.clone();

            let detected = self.run_image(ctx).await?;
            detected_total += detected.len();
            doc.add_entities(detected);
        }

        tracing::debug!(
            target: TARGET,
            detected = detected_total,
            "appending image-detected entities",
        );
        Ok(())
    }

    /// Image-node detection body. Runs text recognizers over each
    /// OCR'd block ("runs alongside" image-side recognizers), then
    /// image recognizers once per image location with raw bytes.
    /// When the `image` feature is off only the text pass runs.
    #[cfg(feature = "image")]
    async fn detect_image(
        &self,
        doc: &mut nvisy_ontology::document::Document<Image>,
        handle: &crate::core::SharedHandle,
        cfg: &Detection,
        run_id: uuid::Uuid,
    ) -> Result<()> {
        detect_text_blocks(self, doc, cfg, run_id).await?;
        self.detect_image_locations(doc, handle, cfg, run_id)
            .await?;
        self.reset().await;
        Ok(())
    }

    #[cfg(not(feature = "image"))]
    async fn detect_image(
        &self,
        doc: &mut nvisy_ontology::document::Document<Image>,
        _handle: &crate::core::SharedHandle,
        cfg: &Detection,
        run_id: uuid::Uuid,
    ) -> Result<()> {
        self.detect_text_only(doc, cfg, run_id).await
    }

    /// Per-node dispatch: route a [`NodeMut`] to the matching
    /// per-modality detection body on `self`. Called once per node
    /// by [`DetectionPhase::apply`].
    ///
    /// Text / Tabular / Audio share the [`Self::detect_text_only`]
    /// pass; Image runs text recognizers ("runs alongside") and
    /// then the image recognizers via
    /// [`Self::detect_image_locations`] when the `image` feature
    /// is on, falling back to text-only otherwise.
    ///
    /// [`NodeMut`]: crate::core::NodeMut
    /// [`DetectionPhase::apply`]: super::DetectionPhase::apply
    pub(super) async fn dispatch(
        &self,
        node: crate::core::NodeMut<'_>,
        handle: &crate::core::SharedHandle,
        cfg: &Detection,
        run_id: uuid::Uuid,
    ) -> Result<()> {
        match node {
            crate::core::NodeMut::Text(doc) => {
                self.detect_text_only::<Text>(doc, cfg, run_id).await
            }
            crate::core::NodeMut::Tabular(doc) => {
                self.detect_text_only::<nvisy_ontology::modality::Tabular>(doc, cfg, run_id)
                    .await
            }
            crate::core::NodeMut::Audio(doc) => {
                self.detect_text_only::<nvisy_ontology::modality::Audio>(doc, cfg, run_id)
                    .await
            }
            crate::core::NodeMut::Image(doc) => self.detect_image(doc, handle, cfg, run_id).await,
        }
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

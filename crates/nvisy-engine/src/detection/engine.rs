//! [`DetectionEngine`]: the per-modality recognizer registry +
//! dispatcher.
//!
//! Holds two `Vec<(String, Arc<dyn _Recognizer>)>` registries — one
//! per modality — and dispatches each registered recognizer against
//! a per-modality context. Built-in recognizers are registered under
//! the stable [`names`] constants ([`names::PATTERN`], [`names::LLM`],
//! [`names::NER`], [`names::VLM`]); operators can append custom
//! recognizers under any unique name they choose via
//! [`DetectionEngine::add_text_recognizer`] / [`add_image_recognizer`].
//!
//! Per-run filtering is name-based: [`Detection::kinds`] is a
//! `Vec<String>` allowlist. Empty means "run every registered
//! recognizer". Names that don't match any registered recognizer are
//! warn-logged at dispatch and silently skipped — matching Presidio's
//! lenient `entities=[...]` semantics.
//!
//! Each recognizer runs on its own [`JoinSet`] task so CPU-bound work
//! (pattern) and I/O-bound work (LLM/NER/VLM) overlap across worker
//! threads. Failure is fail-fast within a modality: on the first task
//! error every other in-flight task in that modality is aborted and
//! the error is returned.
//!
//! [`DetectionPhase`]: super::DetectionPhase
//! [`Detection::kinds`]: super::Detection::kinds
//! [`JoinSet`]: tokio::task::JoinSet
//! [`names`]: super::recognizer::names
//! [`add_image_recognizer`]: DetectionEngine::add_image_recognizer

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use nvisy_agent::agent::NerHint;
use nvisy_core::{Error, Result};
use nvisy_ontology::document::{Document, Span};
use nvisy_ontology::entity::{Annotation, AnnotationKind, AnnotationStrength, Entity};
use nvisy_ontology::modality::{Audio, Image, Modality, ModalityBlock, Overlap, Tabular, Text};
use tokio::task::JoinSet;
use tracing::Instrument;

use super::config::DetectionConfig;
use super::context::{ImageDetectionContext, TextDetectionContext};
use super::lift::{LiftFromBlock, ProjectIntoBlock};
use super::llm::build_recognizer as build_llm_recognizer;
use super::ner::NerRecognizer;
use super::plan::Detection;
use super::recognizer::{ImageRecognizer, TextRecognizer, names};
use super::vlm::build_recognizer as build_vlm_recognizer;
use crate::core::{NodeMut, SharedHandle};

pub(super) const TARGET: &str = "nvisy_engine::detection";

/// Name-based registry + dispatcher.
///
/// Each modality keeps an ordered `Vec<(name, recognizer)>`; iteration
/// order matches registration order. Built once at startup from a
/// [`DetectionConfig`] (which registers built-ins for each opted-in
/// `[detection.*]` section), then optionally extended by the operator
/// with [`add_text_recognizer`] / [`add_image_recognizer`].
///
/// [`add_text_recognizer`]: Self::add_text_recognizer
/// [`add_image_recognizer`]: Self::add_image_recognizer
#[derive(Default, Clone)]
pub struct DetectionEngine {
    /// Text-side registry: pairs of `(stable name, recognizer)`.
    pub text: Vec<(String, Arc<dyn TextRecognizer>)>,
    /// Image-side registry: pairs of `(stable name, recognizer)`.
    pub image: Vec<(String, Arc<dyn ImageRecognizer>)>,
}

impl DetectionEngine {
    /// Build an empty engine. Useful for tests; production callers
    /// normally use [`from_config`].
    ///
    /// [`from_config`]: Self::from_config
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the engine once from a [`DetectionConfig`].
    ///
    /// Each opted-in section drives one built-in registration under
    /// the corresponding stable name ([`names::PATTERN`],
    /// [`names::LLM`], [`names::NER`], [`names::VLM`]). Construction
    /// is eager — model loads, HTTP-client setup, and regex
    /// compilation all happen here so per-run dispatch is cheap.
    ///
    /// # Errors
    ///
    /// Returns the first construction error encountered (NER backend
    /// init failure, LLM provider misconfiguration). Pattern
    /// construction is infallible.
    pub async fn from_config(cfg: &DetectionConfig) -> Result<Self> {
        let mut engine = Self::new();

        if let Some(c) = cfg.llm.as_ref().filter(|c| c.enabled) {
            engine.add_text_recognizer(names::LLM, build_llm_recognizer(c.clone())?);
        }
        if let Some(c) = cfg.ner.as_ref().filter(|c| c.enabled) {
            engine.add_text_recognizer(names::NER, Arc::new(NerRecognizer::from_config(c).await?));
        }
        if let Some(c) = cfg.vlm.as_ref().filter(|c| c.enabled) {
            engine.add_image_recognizer(names::VLM, build_vlm_recognizer(c.clone())?);
        }

        Ok(engine)
    }

    /// Register a text-modality recognizer under `name`. Names must
    /// be unique within the text registry; re-registering an
    /// existing name panics.
    pub fn add_text_recognizer(
        &mut self,
        name: impl Into<String>,
        recognizer: Arc<dyn TextRecognizer>,
    ) {
        let name = name.into();
        assert!(
            !self.text.iter().any(|(n, _)| n == &name),
            "duplicate text recognizer name: {name}",
        );
        self.text.push((name, recognizer));
    }

    /// Register an image-modality recognizer under `name`. Names
    /// must be unique within the image registry; re-registering an
    /// existing name panics.
    pub fn add_image_recognizer(
        &mut self,
        name: impl Into<String>,
        recognizer: Arc<dyn ImageRecognizer>,
    ) {
        let name = name.into();
        assert!(
            !self.image.iter().any(|(n, _)| n == &name),
            "duplicate image recognizer name: {name}",
        );
        self.image.push((name, recognizer));
    }

    /// Run the configured text recognizers (filtered by `kinds`)
    /// against `ctx` in parallel and return the combined entity set.
    async fn run_text(
        &self,
        ctx: TextDetectionContext,
        kinds: &[String],
    ) -> Result<Vec<Entity<Text>>> {
        let span = tracing::debug_span!(
            target: TARGET,
            "detect.text",
            text_len = ctx.text.len(),
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        );

        warn_unmatched_kinds(kinds, &self.text);

        let ctx = Arc::new(ctx);
        let mut set: JoinSet<Result<Vec<Entity<Text>>>> = JoinSet::new();

        for (name, recognizer) in &self.text {
            if !name_allowed(kinds, name) {
                continue;
            }
            let recognizer = Arc::clone(recognizer);
            let ctx = Arc::clone(&ctx);
            set.spawn(async move { recognizer.recognize(&ctx).await });
        }

        async move { collect_join_set(set).await }
            .instrument(span)
            .await
    }

    /// Run the configured image recognizers (filtered by `kinds`)
    /// against `ctx` in parallel and return the combined entity set.
    #[cfg(feature = "image")]
    async fn run_image(
        &self,
        ctx: ImageDetectionContext,
        kinds: &[String],
    ) -> Result<Vec<Entity<Image>>> {
        let span = tracing::debug_span!(
            target: TARGET,
            "detect.image",
            image_bytes = ctx.image.len(),
            width = ctx.dims.width,
            height = ctx.dims.height,
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        );

        warn_unmatched_kinds(kinds, &self.image);

        let ctx = Arc::new(ctx);
        let mut set: JoinSet<Result<Vec<Entity<Image>>>> = JoinSet::new();

        for (name, recognizer) in &self.image {
            if !name_allowed(kinds, name) {
                continue;
            }
            let recognizer = Arc::clone(recognizer);
            let ctx = Arc::clone(&ctx);
            set.spawn(async move { recognizer.recognize(&ctx).await });
        }

        async move { collect_join_set(set).await }
            .instrument(span)
            .await
    }

    /// Reset per-document state on every registered recognizer.
    /// Call at document boundaries.
    pub async fn reset(&self) {
        for (_, recognizer) in &self.text {
            recognizer.reset().await;
        }
        for (_, recognizer) in &self.image {
            recognizer.reset().await;
        }
    }
}

/// True when `kinds` is empty (no filter) or contains `name`.
fn name_allowed(kinds: &[String], name: &str) -> bool {
    kinds.is_empty() || kinds.iter().any(|k| k == name)
}

/// Warn-log any names in `kinds` that don't match any registered
/// recognizer. Helps operators catch typos without breaking the run.
fn warn_unmatched_kinds<R: ?Sized>(kinds: &[String], registry: &[(String, Arc<R>)]) {
    if kinds.is_empty() {
        return;
    }
    let registered: HashSet<&str> = registry.iter().map(|(n, _)| n.as_str()).collect();
    for requested in kinds {
        if !registered.contains(requested.as_str()) {
            tracing::warn!(
                target: TARGET,
                name = %requested,
                "plan requested recognizer `{requested}` but no such recognizer is registered",
            );
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
                return Err(Error::runtime(
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
            .field(
                "text",
                &self.text.iter().map(|(n, _)| n).collect::<Vec<_>>(),
            )
            .field(
                "image",
                &self.image.iter().map(|(n, _)| n).collect::<Vec<_>>(),
            )
            .finish()
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
    doc: &mut Document<M>,
    cfg: &Detection,
    run_id: uuid::Uuid,
) -> Result<()>
where
    M: Modality + LiftFromBlock + ProjectIntoBlock + Overlap,
{
    if doc.blocks.is_empty() {
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

        let mut ctx = TextDetectionContext::new(text.to_owned());
        ctx.correlation_id = Some(run_id);
        if !cfg.entity_kinds.is_empty() {
            ctx.entities = Some(cfg.entity_kinds.clone());
        }
        ctx.hints = hints;
        ctx.labels = labels.clone();

        let detected = engine.run_text(ctx, &cfg.kinds).await?;
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
                entity_kind: entity.entity_kind,
                location,
                confidence: entity.confidence,
                trail: entity.trail,
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
        doc: &mut Document<M>,
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
    /// the target's handle, appending detections to `doc.audit`.
    /// Each call sees the raw encoded image bytes plus the image's
    /// pixel dimensions; recognizers emit absolute image-coord
    /// entities directly (no per-block lifting).
    #[cfg(feature = "image")]
    pub(crate) async fn detect_image_locations(
        &self,
        doc: &mut Document<Image>,
        handle: &SharedHandle,
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
            let bytes = image_data
                .encode_png()
                .map_err(|e| Error::runtime(e.to_string(), "detection-engine", false))?;

            let mut ctx = ImageDetectionContext::new(bytes, dims);
            ctx.correlation_id = Some(run_id);
            if !cfg.entity_kinds.is_empty() {
                ctx.entities = Some(cfg.entity_kinds.clone());
            }
            ctx.labels = labels.clone();

            let detected = self.run_image(ctx, &cfg.kinds).await?;
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
        doc: &mut Document<Image>,
        handle: &SharedHandle,
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
        doc: &mut Document<Image>,
        _handle: &SharedHandle,
        cfg: &Detection,
        run_id: uuid::Uuid,
    ) -> Result<()> {
        self.detect_text_only(doc, cfg, run_id).await
    }

    /// Per-node dispatch: route a [`NodeMut`] to the matching
    /// per-modality detection body on `self`.
    pub(super) async fn dispatch(
        &self,
        node: NodeMut<'_>,
        handle: &SharedHandle,
        cfg: &Detection,
        run_id: uuid::Uuid,
    ) -> Result<()> {
        match node {
            NodeMut::Text(doc) => self.detect_text_only::<Text>(doc, cfg, run_id).await,
            NodeMut::Tabular(doc) => self.detect_text_only::<Tabular>(doc, cfg, run_id).await,
            NodeMut::Audio(doc) => self.detect_text_only::<Audio>(doc, cfg, run_id).await,
            NodeMut::Image(doc) => self.detect_image(doc, handle, cfg, run_id).await,
        }
    }
}

/// Collect [`NerHint`]s for a single block: walk every
/// [`Hint`]-strength [`Inclusion`] annotation, project each one
/// onto the block's text-byte coordinates via [`ProjectIntoBlock`],
/// and emit a hint when the projection succeeds.
///
/// [`Hint`]: AnnotationStrength::Hint
/// [`Inclusion`]: AnnotationKind::Inclusion
fn collect_hints_for_block<M>(annotations: &[Annotation<M>], spans: &[Span<M>]) -> Vec<NerHint>
where
    M: Modality + Overlap + ProjectIntoBlock,
{
    annotations
        .iter()
        .filter_map(|ann| {
            let AnnotationKind::Inclusion {
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
                entity_kind: *entity_kind,
                start,
                end,
            })
        })
        .collect()
}

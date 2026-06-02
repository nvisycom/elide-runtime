//! [`RecognizerRegistry`]: per-modality recognizer container +
//! dispatcher.
//!
//! Holds two `Vec<Arc<dyn Recognizer<M>>>` registries — one per
//! modality. Every registered recognizer runs on every dispatch;
//! there is no per-request name-based allowlist. Operators shape the
//! result set by tuning what they register at engine startup
//! (built-ins through [`DetectionConfig`], custom recognizers through
//! [`add_text_recognizer`] / [`add_image_recognizer`]) and by
//! filtering downstream via [`Detection::entity_kinds`].
//!
//! Beyond registration this type also drives dispatch: per-modality
//! [`Context`] construction, fan-out via [`JoinSet`] (CPU-bound
//! pattern + I/O-bound NER overlap across worker threads), entity
//! lifting from block-local offsets to modality coordinates, image
//! location iteration, and per-document reset. Presidio's
//! `RecognizerRegistry` covers only the container part; the rest sits
//! here for now because no separate "analyzer" layer has earned its
//! keep.
//!
//! Failure is fail-fast within a modality: on the first task error
//! every other in-flight task in that modality is aborted and the
//! error is returned.
//!
//! [`Detection::entity_kinds`]: super::Detection::entity_kinds
//! [`JoinSet`]: tokio::task::JoinSet
//! [`add_text_recognizer`]: RecognizerRegistry::add_text_recognizer
//! [`add_image_recognizer`]: RecognizerRegistry::add_image_recognizer
//! [`Context`]: nvisy_core::Context

use std::fmt;
use std::sync::Arc;

use nvisy_core::{Context, Error, ImageData, Recognizer, Result, TextData};
use nvisy_ontology::document::Document;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::{Audio, Image, Modality, ModalityBlock, Overlap, Tabular, Text};
use tokio::task::JoinSet;
use tracing::Instrument;

use super::lift::{LiftFromBlock, ProjectIntoBlock};
use super::ner::build_recognizer as build_ner_recognizer;
use super::pattern::build_recognizer as build_pattern_recognizer;
use crate::core::{NodeMut, SharedHandle};
use crate::pipeline::{Detection, DetectionConfig};

pub(super) const TARGET: &str = "nvisy_engine::detection";

/// Per-modality recognizer container + dispatcher.
///
/// Each modality keeps an ordered `Vec<Arc<dyn Recognizer<M>>>`;
/// iteration order matches registration order. Built once at startup
/// from a [`DetectionConfig`] (which registers built-ins for each
/// opted-in `[detection.*]` section), then optionally extended by
/// the operator with [`add_text_recognizer`] / [`add_image_recognizer`].
///
/// [`add_text_recognizer`]: Self::add_text_recognizer
/// [`add_image_recognizer`]: Self::add_image_recognizer
#[derive(Default, Clone)]
pub struct RecognizerRegistry {
    /// Text-modality recognizers, dispatched in registration order.
    pub text: Vec<Arc<dyn Recognizer<Text>>>,
    /// Image-modality recognizers, dispatched in registration order.
    pub image: Vec<Arc<dyn Recognizer<Image>>>,
}

impl RecognizerRegistry {
    /// Build an empty registry. Useful for tests; production callers
    /// normally use [`from_config`].
    ///
    /// [`from_config`]: Self::from_config
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the registry from a [`DetectionConfig`].
    ///
    /// Pattern detection is always-on: even when `cfg.pattern` is
    /// `None`, a pattern recognizer with the shipped default registry
    /// is registered. NER is opt-in via `cfg.ner`.
    ///
    /// # Errors
    ///
    /// Returns the first construction error encountered — pattern
    /// compile failure (would be a bug in `nvisy-pattern`'s shipped
    /// patterns), NER backend init failure (e.g. invalid Bento base
    /// URL), or a config-selected backend whose feature wasn't
    /// compiled in.
    pub async fn from_config(cfg: &DetectionConfig) -> Result<Self> {
        let mut registry = Self::new();

        let pattern_cfg = cfg.pattern.clone().unwrap_or_default();
        if pattern_cfg.enabled {
            registry.add_text_recognizer(build_pattern_recognizer(&pattern_cfg)?);
        }

        if let Some(c) = cfg.ner.as_ref().filter(|c| c.enabled) {
            registry.add_text_recognizer(build_ner_recognizer(c)?);
        }

        Ok(registry)
    }

    /// Register a text-modality recognizer. Appended to the existing
    /// list; iteration order at dispatch matches registration order.
    pub fn add_text_recognizer(&mut self, recognizer: Arc<dyn Recognizer<Text>>) {
        self.text.push(recognizer);
    }

    /// Register an image-modality recognizer. Appended to the
    /// existing list; iteration order at dispatch matches registration
    /// order.
    pub fn add_image_recognizer(&mut self, recognizer: Arc<dyn Recognizer<Image>>) {
        self.image.push(recognizer);
    }

    /// Run every registered text recognizer against `ctx` in parallel
    /// and return the combined entity set.
    async fn run_text(&self, ctx: Context<TextData>) -> Result<Vec<Entity<Text>>> {
        let span = tracing::debug_span!(
            target: TARGET,
            "detect.text",
            text_len = ctx.data.text.len(),
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        );

        let ctx = Arc::new(ctx);
        let mut set: JoinSet<Result<Vec<Entity<Text>>>> = JoinSet::new();

        for recognizer in &self.text {
            let recognizer = Arc::clone(recognizer);
            let ctx = Arc::clone(&ctx);
            set.spawn(async move { recognizer.recognize(&ctx).await });
        }

        async move { collect_join_set(set).await }
            .instrument(span)
            .await
    }

    /// Run every registered image recognizer against `ctx` in
    /// parallel and return the combined entity set.
    #[cfg(feature = "image")]
    async fn run_image(&self, ctx: Context<ImageData>) -> Result<Vec<Entity<Image>>> {
        let span = tracing::debug_span!(
            target: TARGET,
            "detect.image",
            image_bytes = ctx.data.bytes.len(),
            width = ctx.data.dims.width,
            height = ctx.data.dims.height,
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        );

        let ctx = Arc::new(ctx);
        let mut set: JoinSet<Result<Vec<Entity<Image>>>> = JoinSet::new();

        for recognizer in &self.image {
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
                return Err(Error::runtime(
                    format!("recognizer task panicked or was cancelled: {join_err}"),
                    "recognizer-registry",
                    false,
                ));
            }
        }
    }
    Ok(all)
}

impl fmt::Debug for RecognizerRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecognizerRegistry")
            .field("text", &self.text.len())
            .field("image", &self.image.len())
            .finish()
    }
}

/// Shared text-side block loop. Used by every modality that exposes
/// text via `ModalityBlock::scan_text` (today: every modality).
///
/// Recursion into [`TextBlock::Embed`] children is the orchestrator's
/// job; this loop scans the target's *own* blocks only and skips
/// embeds via `scan_text` returning `None`.
///
/// [`TextBlock::Embed`]: nvisy_ontology::modality::TextBlock::Embed
pub(crate) async fn detect_text_blocks<M>(
    registry: &RecognizerRegistry,
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

    for block in &doc.blocks {
        let Some(text) = block.kind.scan_text() else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        scanned_blocks += 1;

        let mut ctx = Context::new(TextData::new(text.to_owned()));
        ctx.correlation_id = Some(run_id);

        let detected = registry.run_text(ctx).await?;
        for entity in detected {
            // Centralized entity-kind allowlist filter.
            if !cfg.entity_kinds.is_empty() && !cfg.entity_kinds.contains(&entity.entity_kind) {
                continue;
            }
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

impl RecognizerRegistry {
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
    /// the target's handle, appending detections to `doc.audit`. Each
    /// call sees the raw encoded image bytes plus the image's pixel
    /// dimensions; recognizers emit absolute image-coord entities
    /// directly (no per-block lifting).
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
                .map_err(|e| Error::runtime(e.to_string(), "recognizer-registry", false))?;

            let mut ctx = Context::new(ImageData::new(bytes, dims));
            ctx.correlation_id = Some(run_id);

            let detected = self.run_image(ctx).await?;
            let filtered: Vec<Entity<Image>> = detected
                .into_iter()
                .filter(|e| {
                    cfg.entity_kinds.is_empty() || cfg.entity_kinds.contains(&e.entity_kind)
                })
                .collect();
            detected_total += filtered.len();
            doc.add_entities(filtered);
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
    /// image recognizers once per image location with raw bytes. When
    /// the `image` feature is off only the text pass runs.
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

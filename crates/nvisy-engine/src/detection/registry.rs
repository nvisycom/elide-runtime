//! [`RecognizerRegistry`]: per-modality recognizer container.
//!
//! Two ordered `Vec<Arc<dyn EntityRecognizer<M>>>` lists, one per
//! modality. Every registered recognizer runs on every dispatch;
//! there is no per-request name-based allowlist. Operators shape
//! the result set by tuning what they register at engine startup
//! (built-ins through [`DetectionConfig`], custom recognizers
//! through [`add_text_recognizer`] / [`add_image_recognizer`]) and
//! by filtering downstream via [`Detection::entity_kinds`].
//!
//! Scope: this type knows nothing about [`Document`]. It only owns
//! recognizers and runs them against a [`Context`]. Walking a
//! document, lifting block-local spans to modality coordinates, and
//! per-modality node dispatch all live in [`super::dispatch`].
//!
//! Failure is fail-fast within a modality: on the first task error
//! every other in-flight task in that modality is aborted and the
//! error is returned.
//!
//! [`Detection::entity_kinds`]: crate::pipeline::Detection::entity_kinds
//! [`add_text_recognizer`]: RecognizerRegistry::add_text_recognizer
//! [`add_image_recognizer`]: RecognizerRegistry::add_image_recognizer
//! [`Context`]: nvisy_core::Context
//! [`Document`]: nvisy_ontology::document::Document
//! [`DetectionConfig`]: crate::pipeline::DetectionConfig

use std::fmt;
use std::sync::Arc;

use nvisy_core::{Context, EntityRecognizer, Error, ImageData, Result, TextData};
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::{Image, Modality, Text};
use tokio::task::JoinSet;
use tracing::Instrument;

use crate::pipeline::DetectionConfig;

const TARGET: &str = "nvisy_engine::detection";

/// Per-modality recognizer container.
///
/// Each modality keeps an ordered `Vec<Arc<dyn EntityRecognizer<M>>>`;
/// iteration order matches registration order. Built once at startup
/// from a [`DetectionConfig`] (which builds each opted-in
/// `[detection.*]` section), then optionally extended by the operator
/// with [`add_text_recognizer`] / [`add_image_recognizer`].
///
/// [`add_text_recognizer`]: Self::add_text_recognizer
/// [`add_image_recognizer`]: Self::add_image_recognizer
#[derive(Default, Clone)]
pub struct RecognizerRegistry {
    /// Text-modality recognizers, dispatched in registration order.
    pub text: Vec<Arc<dyn EntityRecognizer<Text>>>,
    /// Image-modality recognizers, dispatched in registration order.
    pub image: Vec<Arc<dyn EntityRecognizer<Image>>>,
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
            registry.add_text_recognizer(pattern_cfg.build()?);
        }

        if let Some(c) = cfg.ner.as_ref().filter(|c| c.enabled) {
            registry.add_text_recognizer(c.build()?);
        }

        Ok(registry)
    }

    /// Register a text-modality recognizer. Appended to the existing
    /// list; iteration order at dispatch matches registration order.
    pub fn add_text_recognizer(&mut self, recognizer: Arc<dyn EntityRecognizer<Text>>) {
        self.text.push(recognizer);
    }

    /// Register an image-modality recognizer. Appended to the
    /// existing list; iteration order at dispatch matches
    /// registration order.
    pub fn add_image_recognizer(&mut self, recognizer: Arc<dyn EntityRecognizer<Image>>) {
        self.image.push(recognizer);
    }

    /// Run every registered text recognizer against `ctx` in parallel
    /// and return the combined entity set.
    pub(crate) async fn run_text(&self, ctx: Context<TextData>) -> Result<Vec<Entity<Text>>> {
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
    pub(crate) async fn run_image(&self, ctx: Context<ImageData>) -> Result<Vec<Entity<Image>>> {
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

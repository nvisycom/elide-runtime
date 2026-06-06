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
//! recognizers and runs them against a [`RecognizerInput`]. Walking a
//! document, lifting block-local spans to modality coordinates, and
//! per-modality node dispatch all live in [`super::dispatch`].
//!
//! Failure is fail-fast within a modality: on the first task error
//! every other in-flight task in that modality is aborted and the
//! error is returned.
//!
//! [`Detection::entity_kinds`]: crate::detection::Detection::entity_kinds
//! [`add_text_recognizer`]: RecognizerRegistry::add_text_recognizer
//! [`add_image_recognizer`]: RecognizerRegistry::add_image_recognizer
//! [`RecognizerInput`]: nvisy_core::recognition::RecognizerInput
//! [`Document`]: nvisy_document::document::Document
//! [`DetectionConfig`]: crate::detection::DetectionConfig

use std::fmt;
use std::sync::Arc;

use nvisy_core::entity::Entity;
use nvisy_core::modality::{Image, Modality, Text};
use nvisy_core::recognition::{EntityRecognizer, RecognizerInput, RecognizerOutput};
use nvisy_core::{Error, Result};
use tokio::task::JoinSet;
use tracing::Instrument;

const TARGET: &str = "nvisy_document::detection";

/// Per-modality recognizer container.
///
/// Each modality keeps an ordered `Vec<Arc<dyn EntityRecognizer<M>>>`;
/// iteration order matches registration order. Built once at startup
/// from `DetectionConfig::build` (which builds each opted-in
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
    /// Build an empty registry. Consumers populate it with
    /// [`add_text_recognizer`] / [`add_image_recognizer`] after
    /// constructing each backend they want to install. Config-driven
    /// construction (TOML → concrete backends) lives in the pipeline
    /// layer of `nvisy-document`, not here.
    ///
    /// [`add_text_recognizer`]: Self::add_text_recognizer
    /// [`add_image_recognizer`]: Self::add_image_recognizer
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a text-modality recognizer. Appended to the existing
    /// list; iteration order at dispatch matches registration order.
    /// Chainable — returns `self` for builder-style construction.
    /// Takes ownership of the recognizer and wraps it in `Arc`.
    #[must_use]
    pub fn add_text_recognizer<R>(mut self, recognizer: R) -> Self
    where
        R: EntityRecognizer<Text> + 'static,
    {
        self.text.push(Arc::new(recognizer));
        self
    }

    /// Register an image-modality recognizer. Appended to the
    /// existing list; iteration order at dispatch matches
    /// registration order. Chainable — returns `self` for
    /// builder-style construction. Takes ownership and wraps in
    /// `Arc`.
    #[must_use]
    pub fn add_image_recognizer<R>(mut self, recognizer: R) -> Self
    where
        R: EntityRecognizer<Image> + 'static,
    {
        self.image.push(Arc::new(recognizer));
        self
    }

    /// Run every registered text recognizer against `input` in
    /// parallel and return the combined entity set.
    pub async fn run_text(&self, input: RecognizerInput<Text>) -> Result<Vec<Entity<Text>>> {
        let span = tracing::debug_span!(
            target: TARGET,
            "detect.text",
            text_len = input.data.text.len(),
            correlation_id = input.correlation_id.as_ref().map(|id| id.to_string()),
        );

        let input = Arc::new(input);
        let mut set: JoinSet<Result<RecognizerOutput<Text>>> = JoinSet::new();

        for recognizer in &self.text {
            let recognizer = Arc::clone(recognizer);
            let input = Arc::clone(&input);
            set.spawn(async move { recognizer.recognize(&input).await });
        }

        async move { collect_join_set(set).await }
            .instrument(span)
            .await
    }

    /// Run every registered image recognizer against `input` in
    /// parallel and return the combined entity set.
    #[cfg(feature = "image")]
    pub async fn run_image(&self, input: RecognizerInput<Image>) -> Result<Vec<Entity<Image>>> {
        let span = tracing::debug_span!(
            target: TARGET,
            "detect.image",
            image_bytes = input.data.bytes.len(),
            width = input.data.dims.width,
            height = input.data.dims.height,
            correlation_id = input.correlation_id.as_ref().map(|id| id.to_string()),
        );

        let input = Arc::new(input);
        let mut set: JoinSet<Result<RecognizerOutput<Image>>> = JoinSet::new();

        for recognizer in &self.image {
            let recognizer = Arc::clone(recognizer);
            let input = Arc::clone(&input);
            set.spawn(async move { recognizer.recognize(&input).await });
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
    mut set: JoinSet<Result<RecognizerOutput<E>>>,
) -> Result<Vec<Entity<E>>> {
    let mut all = Vec::new();
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok(Ok(output)) => {
                tracing::debug!(
                    target: TARGET,
                    detected = output.entities.len(),
                    "recognizer produced entities",
                );
                all.extend(output.entities);
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

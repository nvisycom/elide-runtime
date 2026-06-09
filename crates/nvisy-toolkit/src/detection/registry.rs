//! [`RecognizerRegistry`]: open per-modality recognizer container.
//!
//! One [`TypeMap`] slot per [`Modality`], keyed by the marker
//! type. Each slot is `Vec<Arc<dyn EntityRecognizer<M>>>`; iteration
//! order at dispatch matches registration order. Populate via
//! [`with_recognizer`] after constructing each backend.
//!
//! Scope: this type knows nothing about [`Document`]. It only owns
//! recognizers and runs them against a [`RecognizerInput`]. Walking a
//! document, lifting block-local spans to modality coordinates, and
//! per-modality node dispatch live in the engine's detection phase.
//!
//! Failure is fail-fast within a modality: on the first task error
//! every other in-flight task in that modality is aborted and the
//! error is returned.
//!
//! [`with_recognizer`]: RecognizerRegistry::with_recognizer
//! [`Modality`]: nvisy_core::modality::Modality
//! [`RecognizerInput`]: nvisy_core::recognition::RecognizerInput
//! [`TypeMap`]: type_map::concurrent::TypeMap

use std::fmt;
use std::sync::Arc;

use nvisy_core::entity::Entity;
use nvisy_core::modality::Modality;
use nvisy_core::recognition::{EntityRecognizer, RecognizerInput, RecognizerOutput};
use nvisy_core::{Error, Result};
use tokio::task::JoinSet;
use tracing::Instrument;
use type_map::concurrent::TypeMap;

const TARGET: &str = "nvisy_toolkit::detection";

/// Open per-modality recognizer container.
///
/// Holds a [`TypeMap`] keyed by the modality marker type `M`.
/// Each slot is a `Vec<Arc<dyn EntityRecognizer<M>>>`. Adding a
/// new modality requires zero edits to the registry — the modality
/// crate's `impl Modality` is all that's needed.
///
/// [`TypeMap`]: type_map::concurrent::TypeMap
#[derive(Default)]
pub struct RecognizerRegistry {
    slots: TypeMap,
}

impl RecognizerRegistry {
    /// Empty registry. Populate with [`with_recognizer`].
    ///
    /// [`with_recognizer`]: Self::with_recognizer
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a recognizer for modality `M`. Appended to that
    /// modality's slot; iteration order at dispatch matches
    /// registration order. Chainable.
    #[must_use]
    pub fn with_recognizer<M: Modality>(
        mut self,
        recognizer: impl EntityRecognizer<M> + 'static,
    ) -> Self {
        self.slot_mut::<M>().push(Arc::new(recognizer));
        self
    }

    /// Number of recognizers registered for modality `M`.
    #[must_use]
    pub fn count<M: Modality>(&self) -> usize {
        self.slots
            .get::<Slot<M>>()
            .map_or(0, |slot| slot.recognizers.len())
    }

    /// Run every registered recognizer for modality `M` against
    /// `input` in parallel and return the combined entity set.
    ///
    /// Returns `Ok(Vec::new())` when no recognizers are registered
    /// for `M`.
    pub async fn run<M>(&self, input: RecognizerInput<M>) -> Result<Vec<Entity<M>>>
    where
        M: Modality,
        M::Data: fmt::Debug,
    {
        let Some(slot) = self.slots.get::<Slot<M>>() else {
            return Ok(Vec::new());
        };
        if slot.recognizers.is_empty() {
            return Ok(Vec::new());
        }

        let span = tracing::debug_span!(
            target: TARGET,
            "detect",
            modality = M::NAME,
            input = ?input.data,
            correlation_id = input.correlation_id.as_ref().map(|id| id.to_string()),
        );

        let input = Arc::new(input);
        let mut set: JoinSet<Result<RecognizerOutput<M>>> = JoinSet::new();
        for recognizer in &slot.recognizers {
            let recognizer = Arc::clone(recognizer);
            let input = Arc::clone(&input);
            set.spawn(async move { recognizer.recognize(&input).await });
        }

        async move { collect_join_set::<M>(set).await }
            .instrument(span)
            .await
    }

    fn slot_mut<M: Modality>(&mut self) -> &mut Vec<Arc<dyn EntityRecognizer<M>>> {
        &mut self
            .slots
            .entry::<Slot<M>>()
            .or_insert_with(Slot::default)
            .recognizers
    }
}

/// `TypeMap` entry for one modality. Wrapping `Vec` in a named
/// struct keeps the entry shape `Default + Send + Sync` without
/// needing those bounds in the `EntityRecognizer<M>` super-traits.
struct Slot<M: Modality> {
    recognizers: Vec<Arc<dyn EntityRecognizer<M>>>,
}

impl<M: Modality> Default for Slot<M> {
    fn default() -> Self {
        Self {
            recognizers: Vec::new(),
        }
    }
}

async fn collect_join_set<M: Modality>(
    mut set: JoinSet<Result<RecognizerOutput<M>>>,
) -> Result<Vec<Entity<M>>> {
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
        f.debug_struct("RecognizerRegistry").finish_non_exhaustive()
    }
}

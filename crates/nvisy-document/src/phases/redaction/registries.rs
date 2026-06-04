//! [`RedactionRegistries`]: per-modality bundle of
//! [`RedactionRegistry<M>`] instances the deployment code populates
//! at engine startup and the redaction phase consults at apply time.
//!
//! Text / Image / Audio each get a slot. Tabular is omitted: it has
//! no `ModalityData` impl in `nvisy-core`, so `Anonymizer<Tabular>`
//! is not implementable today.
//!
//! [`RedactionRegistry<M>`]: nvisy_toolkit::redaction::RedactionRegistry

use nvisy_core::modality::{Audio, Image, Text};
use nvisy_toolkit::redaction::RedactionRegistry;

/// Per-modality bundle of [`RedactionRegistry<M>`] instances. Built
/// once at engine startup (deployment code registers custom
/// operators) and shared with each [`super::phase::RedactionPhase`].
///
/// [`RedactionRegistry<M>`]: nvisy_toolkit::redaction::RedactionRegistry
#[derive(Clone, Default, Debug)]
pub struct RedactionRegistries {
    /// Custom text-modality operators.
    pub text: RedactionRegistry<Text>,
    /// Custom image-modality operators.
    pub image: RedactionRegistry<Image>,
    /// Custom audio-modality operators.
    pub audio: RedactionRegistry<Audio>,
}

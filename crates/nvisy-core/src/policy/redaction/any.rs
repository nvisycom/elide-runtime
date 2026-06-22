//! [`AnyRedaction`]: type-erased redaction operator spec.
//!
//! Carries the modality tag and the typed [`*Redaction`] enum that
//! a policy author would pick. Used at API boundaries where the
//! caller hasn't pinned an `M: Modality` type yet — most notably
//! [`RedactionOverride::Replace`] and [`RedactionAddEntity`] in the
//! redaction subsystem.
//!
//! Wire shape:
//!
//! ```json
//! { "modality": "text", "kind": "mask", ... }
//! ```
//!
//! [`RedactionOverride::Replace`]: crate::redaction::RedactionOverride::Replace
//! [`RedactionAddEntity`]: crate::redaction::RedactionAddEntity

use nvisy_core::modality::ModalityKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::EnumTryAs;

use super::audio::AudioRedaction;
use super::image::ImageRedaction;
use super::tabular::TabularRedaction;
use super::text::TextRedaction;

/// Type-erased redaction operator spec.
///
/// Use the typed [`TextRedaction`] / [`ImageRedaction`] /
/// [`AudioRedaction`] / [`TabularRedaction`] enums directly inside
/// policy rules where the modality is known at compile time;
/// reach for `AnyRedaction` only at the dynamic API boundary.
#[derive(Debug, Clone, PartialEq, Eq, EnumTryAs)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "modality", rename_all = "snake_case")]
pub enum AnyRedaction {
    /// Text-modality redaction operator.
    Text(TextRedaction),
    /// Tabular-modality redaction operator.
    Tabular(TabularRedaction),
    /// Image-modality redaction operator.
    Image(ImageRedaction),
    /// Audio-modality redaction operator.
    Audio(AudioRedaction),
}

impl AnyRedaction {
    /// The modality this redaction operator belongs to.
    #[must_use]
    pub fn modality(&self) -> ModalityKind {
        match self {
            Self::Text(_) => ModalityKind::Text,
            Self::Tabular(_) => ModalityKind::Tabular,
            Self::Image(_) => ModalityKind::Image,
            Self::Audio(_) => ModalityKind::Audio,
        }
    }
}

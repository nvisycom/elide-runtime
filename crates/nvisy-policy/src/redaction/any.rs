//! [`AnyRedaction`]: type-erased redaction operator spec.
//!
//! Carries the modality tag and the typed per-modality `*Redaction`
//! enum that a policy author would pick. Used at API boundaries
//! where the caller has not pinned an `M: Modality` type yet, most
//! notably the redaction-override surface (`Replace`, `Add`).
//!
//! Wire shape:
//!
//! ```json
//! { "modality": "text", "kind": "mask", ... }
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::EnumTryAs;

use super::audio::AudioRedaction;
use super::image::ImageRedaction;
use super::tabular::TabularRedaction;
use super::text::TextRedaction;

/// Which modality an [`AnyRedaction`] applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RedactionModality {
    /// Text-modality entities (txt, json, html, xml, …).
    Text,
    /// Tabular-modality entities (csv, xlsx, …).
    Tabular,
    /// Image-modality entities (png, jpeg, tiff, …).
    Image,
    /// Audio-modality entities (wav, mp3, …).
    Audio,
}

/// Type-erased redaction operator spec.
///
/// Use the typed [`TextRedaction`] / [`ImageRedaction`] /
/// [`AudioRedaction`] / [`TabularRedaction`] enums directly inside
/// policy rules where the modality is known at compile time; reach
/// for [`AnyRedaction`] only at the dynamic API boundary.
#[derive(Debug, Clone, PartialEq, EnumTryAs)]
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
    pub fn modality(&self) -> RedactionModality {
        match self {
            Self::Text(_) => RedactionModality::Text,
            Self::Tabular(_) => RedactionModality::Tabular,
            Self::Image(_) => RedactionModality::Image,
            Self::Audio(_) => RedactionModality::Audio,
        }
    }
}

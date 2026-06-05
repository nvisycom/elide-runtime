//! [`Redactable`]: redaction-side extension trait on
//! [`nvisy_core::modality::Modality`].
//!
//! Names the per-modality `Replacement` type — the value an
//! [`Anonymizer<M>`] emits and the document phase writes back at the
//! entity's location.
//!
//! [`Anonymizer<M>`]: super::Anonymizer

use std::fmt::Debug;

use nvisy_core::modality::{Audio, Image, Modality, Tabular, Text};
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::{AudioReplacement, ImageReplacement, TabularReplacement, TextReplacement};

/// Redaction-side extension of [`Modality`].
///
/// Names the per-modality replacement record an [`Anonymizer<M>`]
/// emits. The `ModalityData` bound that lets [`Anonymizer<M>::apply`]
/// borrow the source payload lives on the [`Anonymizer<M>`] trait,
/// not here — that keeps `Tabular` (which has no `ModalityData`
/// impl in core today) usable in policy / audit type signatures
/// downstream even before a tabular operator ships.
///
/// The `Replacement` associated type carries the full
/// `Serialize + DeserializeOwned + JsonSchema` bundle so audit
/// records that embed `M::Replacement` derive their codecs without
/// repeating `#[serde(bound = …)]` on every site.
///
/// [`Anonymizer<M>`]: super::Anonymizer
/// [`Anonymizer<M>::apply`]: super::Anonymizer::apply
pub trait Redactable: Modality {
    /// What an [`Anonymizer<M>`] writes at the entity's location.
    ///
    /// [`Anonymizer<M>`]: super::Anonymizer
    type Replacement: Clone
        + Debug
        + PartialEq
        + Send
        + Sync
        + Serialize
        + DeserializeOwned
        + JsonSchema
        + 'static;
}

impl Redactable for Text {
    type Replacement = TextReplacement;
}

impl Redactable for Image {
    type Replacement = ImageReplacement;
}

impl Redactable for Audio {
    type Replacement = AudioReplacement;
}

impl Redactable for Tabular {
    type Replacement = TabularReplacement;
}

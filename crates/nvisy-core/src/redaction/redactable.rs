//! [`Redactable`]: redaction-side extension trait on [`Modality`].
//!
//! Names the per-modality `Replacement` type — the value an
//! anonymizer emits and any write-back path lands at the entity's
//! location.
//!
//! Lives in core so both the toolkit-side anonymizer trait
//! (`nvisy_toolkit::redaction::Anonymizer`) and the core
//! [`RedactAt<M>`] write-back trait bound on the same type without a
//! toolkit dep.
//!
//! [`Modality`]: crate::modality::Modality
//! [`RedactAt<M>`]: crate::extraction::RedactAt

use std::fmt::Debug;

use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::{AudioReplacement, ImageReplacement, TabularReplacement, TextReplacement};
use crate::modality::{Audio, Image, Modality, Tabular, Text};

/// Redaction-side extension of [`Modality`].
///
/// Names the per-modality replacement record an anonymizer emits.
/// The `ModalityData` bound that lets `Anonymizer::apply` borrow the
/// source payload lives on the toolkit-side trait, not here — that
/// keeps `Tabular` (no `ModalityData` impl in core today) usable in
/// policy / audit type signatures downstream even before a tabular
/// operator ships.
///
/// The `Replacement` associated type carries the full
/// `Serialize + DeserializeOwned + JsonSchema` bundle so audit
/// records that embed `M::Replacement` derive their codecs without
/// repeating `#[serde(bound = …)]` on every site.
///
/// [`Modality`]: crate::modality::Modality
pub trait Redactable: Modality {
    /// What an anonymizer writes at the entity's location.
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

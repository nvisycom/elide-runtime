//! [`ModalityKind`]: runtime tag identifying which [`Modality`] a
//! generic container carries.
//!
//! Used for runtime dispatch where the marker type is erased — at
//! the codec / pipeline boundary where a `Document<M>` or
//! `Handle<M>` has been pushed through an [`UntypedDocumentHandle`]
//! or [`AnyLocation`] arm and the typed `M` is no longer in scope.
//!
//! Lives in codec because codec is the most upstream consumer:
//! [`Format`] advertises one, [`ErasedLoader`] returns one,
//! [`Codable::KIND`] is the typed → erased bridge each per-modality
//! marker supplies. Downstream crates (engine) re-use it via
//! `nvisy_codec::core::ModalityKind`.
//!
//! [`Modality`]: nvisy_core::modality::Modality
//! [`UntypedDocumentHandle`]: crate::document::UntypedDocumentHandle
//! [`AnyLocation`]: # "owned by nvisy-engine"
//! [`Format`]: super::Format
//! [`ErasedLoader`]: super::ErasedLoader
//! [`Codable::KIND`]: super::Codable::KIND

use std::any::TypeId;

use nvisy_core::modality::{Audio, Image, Modality, Tabular, Text};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Runtime tag identifying which [`Modality`] a generic container
/// carries.
///
/// [`Modality`]: nvisy_core::modality::Modality
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModalityKind {
    /// [`Text`] modality.
    ///
    /// [`Text`]: nvisy_core::modality::Text
    Text,
    /// [`Tabular`] modality.
    ///
    /// [`Tabular`]: nvisy_core::modality::Tabular
    Tabular,
    /// [`Image`] modality.
    ///
    /// [`Image`]: nvisy_core::modality::Image
    Image,
    /// [`Audio`] modality.
    ///
    /// [`Audio`]: nvisy_core::modality::Audio
    Audio,
}

impl ModalityKind {
    /// Return the [`ModalityKind`] for a typed `M: Modality` at the
    /// call site.
    ///
    /// Resolved at runtime via [`TypeId`] rather than a
    /// `Modality`-level associated const so adding a new modality
    /// type doesn't force every implementor to advertise a tag.
    /// The match is exhaustive over the four built-in marker types;
    /// an unknown `M` panics.
    #[must_use]
    pub fn of<M: Modality>() -> Self {
        let id = TypeId::of::<M>();
        if id == TypeId::of::<Text>() {
            Self::Text
        } else if id == TypeId::of::<Tabular>() {
            Self::Tabular
        } else if id == TypeId::of::<Image>() {
            Self::Image
        } else if id == TypeId::of::<Audio>() {
            Self::Audio
        } else {
            unreachable!("Modality must be one of Text/Tabular/Image/Audio");
        }
    }
}

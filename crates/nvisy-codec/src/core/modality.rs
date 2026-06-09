//! [`ModalityKind`]: runtime tag identifying which
//! [`Modality`](nvisy_core::modality::Modality) a generic container
//! carries.
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
//! [`UntypedDocumentHandle`]: crate::document::UntypedDocumentHandle
//! [`AnyLocation`]: # "owned by nvisy-engine"
//! [`Format`]: super::Format
//! [`ErasedLoader`]: super::ErasedLoader
//! [`Codable::KIND`]: super::Codable::KIND

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Runtime tag identifying which
/// [`Modality`](nvisy_core::modality::Modality) a generic container
/// carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModalityKind {
    /// [`Text`](nvisy_core::modality::Text) modality.
    Text,
    /// [`Tabular`](nvisy_core::modality::Tabular) modality.
    Tabular,
    /// [`Image`](nvisy_core::modality::Image) modality.
    Image,
    /// [`Audio`](nvisy_core::modality::Audio) modality.
    Audio,
}

//! Content modalities and the redaction vocabulary each one admits.
//!
//! Re-exports elide's modality markers so a policy author names
//! them from one place, and adds [`RedactableModality`]: the
//! type-level pairing of a marker with the redaction spec that is
//! meaningful for it.
//!
//! [`ModalityRedactions`] carries one optional slot per modality,
//! which is the right shape for a policy *rule*: one rule can span
//! a container document whose parts span several modalities. It is
//! the wrong shape wherever the modality is already pinned by an
//! `M: Modality` type parameter, because three of its four slots
//! are then unreachable. A reviewer override on a text entity that
//! names only [`ImageRedaction`] type-checks against that shape and
//! then silently does nothing: the text compile path reads
//! `redactions.text`, finds `None`, and passes the entity through
//! to whatever policy rule matches next.
//!
//! [`RedactableModality`] closes that gap. Each marker maps to the
//! single redaction enum meaningful for it, so a generic holder
//! stores `M::Redaction` instead of the four-slot map and the
//! mismatch stops being representable.
//!
//! [`ModalityRedactions`]: crate::redaction::ModalityRedactions
//! [`ImageRedaction`]: crate::redaction::ImageRedaction

use std::fmt::Debug;

pub use elide_core::modality::Modality;
pub use elide_core::modality::audio::Audio;
pub use elide_core::modality::image::Image;
pub use elide_core::modality::tabular::Tabular;
pub use elide_core::modality::text::Text;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::redaction::{AudioRedaction, ImageRedaction, TabularRedaction, TextRedaction};

/// A [`Modality`] whose entities a policy author can declare a
/// redaction operator for.
///
/// Implemented for the four modalities elide ships. An out-of-tree
/// medium implements it against its own redaction enum.
pub trait RedactableModality: Modality {
    /// The redaction operator spec meaningful for this medium.
    ///
    /// Exactly one enum, not a map: pinning `M` pins the operator
    /// vocabulary, so there is nothing left to select at runtime.
    ///
    /// The bounds are what a holder generic over this type needs to
    /// keep deriving `Debug`, serde, and `JsonSchema` the way the
    /// concrete per-modality enums already do.
    type Redaction: Clone
        + Debug
        + PartialEq
        + Serialize
        + for<'de> Deserialize<'de>
        + JsonSchema
        + Send
        + Sync
        + 'static;
}

impl RedactableModality for Text {
    type Redaction = TextRedaction;
}

impl RedactableModality for Tabular {
    type Redaction = TabularRedaction;
}

impl RedactableModality for Image {
    type Redaction = ImageRedaction;
}

impl RedactableModality for Audio {
    type Redaction = AudioRedaction;
}

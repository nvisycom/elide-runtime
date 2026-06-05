//! Per-modality marker types and the [`Modality`] trait.
//!
//! Generic containers ([`Entity<M>`]) are parameterised over
//! `M: Modality`, where `M` is one of the zero-sized markers
//! [`Text`], [`Image`], [`Audio`], or [`Tabular`]. Each marker's
//! [`Modality::Location`] associated type names the actual
//! coordinate value those containers store ([`TextLocation`],
//! [`ImageLocation`], [`AudioLocation`], [`TabularLocation`]).
//!
//! Splitting the marker from the data lets the type system carry
//! "which modality" cleanly while the data can grow shape (e.g. an
//! image location adding polygon variants) without touching the
//! marker.
//!
//! [`Modality`] is intentionally minimal: marker + location.
//! Extension traits ([`crate::ModalityData`] for the recognizer-side
//! payload type, [`crate::extraction::ModalityExtraction`] for the
//! per-modality provenance enum) live next to the layer that needs
//! them. The document-shape side (`Block`, `Metadata`) lives in
//! `nvisy-document`; the redaction-shape side (`Strategy`,
//! `Replacement`) lives in `nvisy-toolkit`. Each layer adds its own
//! extension trait (`DocumentModality`, `Redactable`) atop this
//! marker — toolkit and document don't pollute core.
//!
//! [`Entity<M>`]: crate::entity::Entity

mod audio;
mod image;
mod tabular;
mod text;

use std::fmt::Debug;

use serde::Serialize;
use serde::de::DeserializeOwned;

pub use self::audio::{Audio, AudioData, AudioExtraction, AudioLocation};
pub use self::image::{Image, ImageData, ImageExtraction, ImageLocation};
pub use self::tabular::{Tabular, TabularExtraction, TabularLocation};
pub use self::text::{ContextWindow, Text, TextData, TextExtraction, TextLocation};

/// Runtime tag identifying which [`Modality`] a generic container
/// carries. Use for runtime dispatch where the marker type is erased
/// (typically at the codec / pipeline boundary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModalityKind {
    /// [`Text`] modality.
    Text,
    /// [`Tabular`] modality.
    Tabular,
    /// [`Image`] modality.
    Image,
    /// [`Audio`] modality.
    Audio,
}

/// Extension of [`Modality`] that adds the per-call payload type
/// recognizers and extractors consume.
pub trait ModalityData: Modality {
    /// Per-call modality-specific payload: the bytes / text /
    /// dimensions a recognizer or extractor actually scans.
    type Data: Debug + Send + Sync;
}

impl ModalityData for Text {
    type Data = TextData;
}

impl ModalityData for Image {
    type Data = ImageData;
}

impl ModalityData for Audio {
    type Data = AudioData;
}

impl ModalityData for Tabular {
    type Data = TextData;
}

/// Marker trait implemented by every per-modality marker type
/// ([`Text`], [`Image`], [`Audio`], [`Tabular`]).
///
/// The associated [`Location`] type names the actual coordinate
/// value carried by generic containers parameterised on `M` —
/// `Entity<M>::location` is `M::Location`, etc. The marker type
/// itself is zero-sized; the data lives behind the associated
/// type.
///
/// [`Location`]: Self::Location
pub trait Modality: Copy + Default + Debug + PartialEq + Eq + Send + Sync + 'static {
    /// Coordinate value carried by generic containers parameterised
    /// on this modality.
    type Location: Clone
        + Debug
        + PartialEq
        + Send
        + Sync
        + Serialize
        + DeserializeOwned
        + schemars::JsonSchema
        + 'static;
}

/// Check whether two coordinates of the same modality overlap.
///
/// Implemented on the per-modality [`Location`] types, not on the
/// markers — the markers carry no data and have nothing to compare.
/// Semantics vary by modality:
/// - **Text**: byte-range interval overlap (`start < other.end && other.start < end`).
/// - **Image**: bounding box intersection.
/// - **Audio**: time span overlap.
/// - **Tabular**: same cell (row + column), with optional intra-cell
///   byte-range check when offsets are present.
///
/// [`Location`]: Modality::Location
pub trait Overlap {
    fn overlaps(&self, other: &Self) -> bool;
}

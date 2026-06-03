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
//! [`Modality`] is intentionally minimal: marker + location +
//! extraction. The document-shape side (`Block`, `Metadata`) lives
//! in `nvisy-document`; the redaction-shape side (`Strategy`,
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

pub use self::audio::{Audio, AudioExtraction, AudioLocation};
pub use self::image::{Image, ImageExtraction, ImageLocation};
pub use self::tabular::{Tabular, TabularExtraction, TabularLocation};
pub use self::text::{ContextWindow, Text, TextExtraction, TextLocation};

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

/// Extension of [`Modality`] that names the per-modality
/// [`Extraction`] enum recording how a document's primary content was
/// produced.
///
/// `M::Extraction` is the value stamped into the document's
/// per-modality metadata at extractor time (e.g. `Document<Image>`'s
/// metadata carries an [`ImageExtraction`]). Generic phase code that
/// needs to stamp extraction provenance writes `M::Extraction`; the
/// concrete enum stays modality-keyed and finite.
///
/// [`Extraction`]: Self::Extraction
pub trait ModalityExtraction: Modality {
    /// Per-modality provenance enum recording how the document was
    /// produced (e.g. [`TextExtraction`] for [`Text`],
    /// [`ImageExtraction`] for [`Image`]).
    type Extraction: Clone + Debug + PartialEq + Send + Sync + 'static;
}

impl ModalityExtraction for Text {
    type Extraction = TextExtraction;
}

impl ModalityExtraction for Image {
    type Extraction = ImageExtraction;
}

impl ModalityExtraction for Audio {
    type Extraction = AudioExtraction;
}

impl ModalityExtraction for Tabular {
    type Extraction = TabularExtraction;
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

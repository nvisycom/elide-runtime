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
//! [`Modality`] bundles the four per-modality associated types every
//! consumer reaches for: [`Location`] (coordinate), [`Data`] (per-call
//! payload the recognizer/extractor scans), [`Replacement`] (redaction
//! record), and [`Extraction`] (per-modality provenance enum stamped
//! onto a document at extractor time). The document-shape side
//! (`Block`, `Metadata`) lives in `nvisy-document`; the codec-side
//! tag (`Codable`) lives in `nvisy-codec`. Each layer adds its own
//! extension trait (`DocumentModality`, `Codable`) atop this marker
//! — toolkit and document don't pollute core.
//!
//! [`Location`]: Modality::Location
//! [`Data`]: Modality::Data
//! [`Replacement`]: Modality::Replacement
//! [`Extraction`]: Modality::Extraction
//!
//! [`Entity<M>`]: crate::entity::Entity

mod audio;
mod image;
mod tabular;
mod text;

use std::fmt::Debug;

use schemars::JsonSchema;
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

/// Marker trait implemented by every per-modality marker type
/// ([`Text`], [`Image`], [`Audio`], [`Tabular`]).
///
/// Bundles the per-modality associated types every consumer reaches
/// for:
///
/// - [`Location`] — coordinate value carried by generic containers
///   parameterised on `M` (`Entity<M>::location` is `M::Location`,
///   etc.). Always present.
/// - [`Data`] — per-call payload (the bytes / text / dimensions a
///   recognizer or extractor actually scans). Tabular shares
///   [`TextData`] with Text since tabular cells are text-shaped at
///   the per-call level.
/// - [`Replacement`] — redaction record an anonymizer emits and any
///   write-back path lands at the entity's location.
/// - [`Extraction`] — per-modality provenance enum stamped onto a
///   `Document<M>`'s metadata at extractor time
///   (`TextExtraction`, `ImageExtraction`, etc.).
///
/// The marker type itself is zero-sized; everything else lives
/// behind the associated types.
///
/// [`Location`]: Self::Location
/// [`Data`]: Self::Data
/// [`Replacement`]: Self::Replacement
/// [`Extraction`]: Self::Extraction
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
        + JsonSchema
        + 'static;

    /// Per-call modality-specific payload: the bytes / text /
    /// dimensions a recognizer or extractor actually scans.
    type Data: Debug + Send + Sync;

    /// What an anonymizer writes at the entity's location. Embedded
    /// in audit records, so it carries the full serde + schemars
    /// bundle to derive its codecs transparently.
    type Replacement: Clone
        + Debug
        + PartialEq
        + Send
        + Sync
        + Serialize
        + DeserializeOwned
        + JsonSchema
        + 'static;

    /// Per-modality provenance enum recording how a [`Document<M>`]'s
    /// primary content was produced (e.g. native text-layer parse vs
    /// OCR'd image-backed page).
    ///
    /// [`Document<M>`]: # "carrier owned by nvisy-document"
    type Extraction: Clone + Debug + PartialEq + Send + Sync + 'static;
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

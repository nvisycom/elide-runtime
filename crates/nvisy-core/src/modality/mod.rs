//! Per-modality coordinate types and the [`Modality`] marker trait.
//!
//! Generic containers ([`Entity<M>`]) are parameterised over
//! `M: Modality`, where `M` is one of [`Text`], [`Image`], [`Audio`],
//! or [`Tabular`]. This enforces at compile time that a recognizer
//! wired for text cannot be passed an audio document.
//!
//! [`Modality`] is intentionally minimal: just a marker trait with
//! the structural bounds every generic container needs. The
//! document-shape side (`Block`, `Metadata`) lives in
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

pub use self::audio::{Audio, AudioExtraction};
pub use self::image::{Image, ImageExtraction};
pub use self::tabular::{Tabular, TabularExtraction};
pub use self::text::{ContextWindow, Text, TextExtraction};

/// Marker trait implemented by every per-modality coordinate type.
///
/// Keeps only the structural bounds every generic container needs
/// (`Clone`, `Debug`, `PartialEq`, thread-safety). Per-layer shape
/// (document blocks/metadata, redaction strategies) lives in
/// extension traits in the layers that own those concerns.
pub trait Modality: Clone + Debug + PartialEq + Send + Sync + 'static {}

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
/// Semantics vary by modality:
/// - **Text**: byte-range interval overlap (`start < other.end && other.start < end`).
/// - **Image**: bounding box intersection.
/// - **Audio**: time span overlap.
/// - **Tabular**: same cell (row + column), with optional intra-cell
///   byte-range check when offsets are present.
pub trait Overlap {
    fn overlaps(&self, other: &Self) -> bool;
}

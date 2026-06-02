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

pub use self::audio::Audio;
pub use self::image::Image;
pub use self::tabular::Tabular;
pub use self::text::{ContextWindow, Text};

/// Marker trait implemented by every per-modality coordinate type.
///
/// Keeps only the structural bounds every generic container needs
/// (`Clone`, `Debug`, `PartialEq`, thread-safety). Per-layer shape
/// (document blocks/metadata, redaction strategies) lives in
/// extension traits in the layers that own those concerns.
pub trait Modality: Clone + Debug + PartialEq + Send + Sync + 'static {}

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

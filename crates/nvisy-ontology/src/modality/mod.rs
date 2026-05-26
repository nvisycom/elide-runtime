//! Per-modality coordinate types and the [`Modality`] marker trait.
//!
//! Generic containers ([`Document`], [`Block`], [`Span`], [`Artefact`],
//! [`Entity`]) are parameterised over `M: Modality`, where `M` is one
//! of [`Text`], [`Image`], [`Audio`], or [`Tabular`]. This enforces at
//! compile time that a recognizer wired for text cannot be passed an
//! audio document.
//!
//! [`Document`]: crate::document::Document
//! [`Block`]: crate::document::Block
//! [`Span`]: crate::document::Span
//! [`Artefact`]: crate::document::Artefact
//! [`Entity`]: crate::entity::Entity

mod any;
mod audio;
mod image;
mod tabular;
mod text;

use std::fmt::Debug;

use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub use self::any::AnyModality;
pub use self::audio::{Audio, AudioBlockKind, AudioBuilder, AudioMetadata};
pub use self::image::{
    Image, ImageArtefact, ImageArtefactKind, ImageBlockKind, ImageBuilder, ImageMetadata,
    PageDimensions,
};
pub use self::tabular::{ColumnHeader, Tabular, TabularBuilder, TabularMetadata};
pub use self::text::{Text, TextBlockKind, TextBuilder, TextMetadata};

/// Marker trait implemented by every per-modality coordinate type.
///
/// The trait bounds match what generic containers need to derive
/// `Serialize`, `Deserialize`, `JsonSchema`, `Clone`, `Debug`,
/// `PartialEq` and to be shared across async boundaries.
///
/// The associated types describe what each modality carries *beyond*
/// the source coordinates: per-block classification, non-textual
/// artefacts, and document-level metadata. Modalities that don't need
/// one of these set it to `()`.
pub trait Modality:
    Clone + Debug + PartialEq + Serialize + DeserializeOwned + JsonSchema + Send + Sync + 'static
{
    /// Per-block classification (e.g. paragraph vs heading for text,
    /// text region vs figure for image). Use `()` when the modality
    /// doesn't have multiple kinds of blocks.
    type BlockKind: Clone
        + Debug
        + PartialEq
        + Serialize
        + DeserializeOwned
        + JsonSchema
        + Send
        + Sync
        + 'static;

    /// Non-textual elements detected alongside text in a block (e.g.
    /// figures, separators for image OCR). Use `()` when the modality
    /// has no notion of artefacts.
    type Artefact: Clone
        + Debug
        + PartialEq
        + Serialize
        + DeserializeOwned
        + JsonSchema
        + Send
        + Sync
        + 'static;

    /// Document-level metadata (e.g. languages, column headers, page
    /// dimensions). Carries `Default` so an empty document can be
    /// constructed.
    type Metadata: Clone
        + Debug
        + Default
        + PartialEq
        + Serialize
        + DeserializeOwned
        + JsonSchema
        + Send
        + Sync
        + 'static;
}

/// Combine two values into one when they can be reconciled.
///
/// Used by collections (e.g. [`Redactions`]) under a merge policy:
/// when two entries collide (per [`Overlap`]), the collection asks
/// both the location and the payload whether they can fuse. Returns
/// `Some(merged)` when the two can be combined (e.g. unioned bounding
/// boxes, identical outputs), `None` when they cannot (e.g. different
/// tabular cells, conflicting replacement strings).
///
/// [`Redactions`]: https://docs.rs/nvisy-codec/latest/nvisy_codec/transform/struct.Redactions.html
pub trait Mergeable: Sized {
    fn try_merge(self, other: Self) -> Option<Self>;
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

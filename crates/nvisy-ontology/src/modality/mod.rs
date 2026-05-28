//! Per-modality coordinate types and the [`Modality`] marker trait.
//!
//! Generic containers ([`Document`], [`Span`], [`Entity`]) are
//! parameterised over `M: Modality`, where `M` is one of [`Text`],
//! [`Image`], [`Audio`], or [`Tabular`]. This enforces at compile
//! time that a recognizer wired for text cannot be passed an audio
//! document.
//!
//! Each modality also defines its own [`Block`](Modality::Block) shape
//! (via the associated type) so block payloads can diverge across
//! modalities. A [`TextBlock`] is a structural variant (paragraph,
//! heading, list item, …) carrying flat text. An [`ImageBlock`]
//! carries a per-variant bounding region plus optional recognized
//! text. An [`AudioBlock`] carries the segment's time span and, for
//! speech, the transcript and speaker. A [`TabularBlock`] carries
//! the row's flat text and 0-based row index.
//!
//! [`Document`]: crate::document::Document
//! [`Span`]: crate::document::Span
//! [`Entity`]: crate::entity::Entity

mod audio;
mod image;
mod tabular;
mod text;

use std::fmt::Debug;

pub use self::audio::{Audio, AudioBlock, AudioMetadata};
pub use self::image::{Image, ImageBlock, ImageMetadata, PageDimensions};
pub use self::tabular::{ColumnHeader, Tabular, TabularBlock, TabularMetadata};
pub use self::text::{ContextWindow, Text, TextBlock, TextMetadata};

/// Marker trait implemented by every per-modality coordinate type.
///
/// The trait keeps only the structural bounds every generic
/// container needs (`Clone`, `Debug`, `PartialEq`, thread-safety).
/// Serialization is intentionally *not* required at the trait level
/// — concrete modality types (`Text`, `Image`, `Audio`, `Tabular`)
/// happen to implement `Serialize`/`Deserialize`/`JsonSchema`, and
/// containers that need them (like [`Entity<M>`], [`Audit<M>`])
/// gate their own serde derives on those bounds at the impl level.
///
/// Associated types describe per-modality shape:
///
/// - [`Block`](Self::Block) — the modality's block variant.
/// - [`Metadata`](Self::Metadata) — document-level metadata
///   (languages, page dimensions, column headers).
///
/// [`Entity<M>`]: crate::entity::Entity
/// [`Audit<M>`]: crate::provenance::Audit
pub trait Modality: Clone + Debug + PartialEq + Send + Sync + 'static {
    /// The modality's block payload. See per-modality types:
    /// [`TextBlock`], [`ImageBlock`], [`AudioBlock`], [`TabularBlock`].
    type Block: ModalityBlock + Clone + Debug + PartialEq + Send + Sync + 'static;

    /// Document-level metadata.
    type Metadata: Clone + Debug + Default + PartialEq + Send + Sync + 'static;

    /// The modality's redaction strategy. Each modality declares the
    /// methods that make sense for its data — text picks
    /// mask/replace/encrypt/etc., image picks blur/block/pixelate,
    /// audio picks silence/remove, tabular picks clear/drop-column.
    type Strategy: RedactionStrategy + Clone + Debug + Default + PartialEq + Send + Sync + 'static;
}

/// Shared per-block surface every modality's block payload exposes.
///
/// This is the trait every [`Modality::Block`] type implements; it
/// collects the modality-agnostic queries the engine needs to drive
/// pipeline stages without matching on the concrete block variant.
///
/// Today the only method is [`scan_text`](Self::scan_text), used by
/// the detection driver to decide whether a block carries text the
/// text-typed recognizers should scan. Future shared block queries
/// (block id, kind tag, block-level confidence accessor) land here.
///
/// `Text` and `Tabular` blocks always carry text; their impls return
/// `Some(_)` unconditionally. Image and audio impls return `None`
/// for non-text variants (figures, silences).
pub trait ModalityBlock {
    /// The text a text-typed recognizer should scan over this block,
    /// or `None` when the block carries no scannable text (image
    /// figures/logos, audio silences).
    fn scan_text(&self) -> Option<&str>;
}

/// Methods every per-modality redaction strategy must expose.
pub trait RedactionStrategy {
    /// Whether the strategy is reversible (the original value can be
    /// recovered from the redacted output).
    fn is_reversible(&self) -> bool;
}

/// Combine two values into one when they can be reconciled.
///
/// Used by deduplication and fusion pipelines: when two entries
/// collide (per [`Overlap`]), the consumer asks both the location
/// and the payload whether they can fuse. Returns `Some(merged)`
/// when the two can be combined (e.g. unioned bounding boxes,
/// identical outputs), `None` when they cannot (e.g. different
/// tabular cells, conflicting replacement strings).
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

//! Per-modality coordinate types and the [`Modality`] marker trait.
//!
//! Generic containers ([`Document`], [`Span`], [`Entity`]) are
//! parameterised over `M: Modality`, where `M` is one of [`Text`],
//! [`Image`], [`Audio`], or [`Tabular`]. This enforces at compile
//! time that a recognizer wired for text cannot be passed an audio
//! document.
//!
//! Each modality also defines its own [`Block`] shape
//! (via the associated type) so block payloads can diverge across
//! modalities. A [`TextBlock`] is a structural variant (paragraph,
//! heading, list item, …) carrying flat text. An [`ImageBlock`]
//! carries a per-variant bounding region plus optional recognized
//! text. An [`AudioBlock`] carries the segment's time span and, for
//! speech, the transcript and speaker. A [`TabularBlock`] carries
//! the row's flat text and 0-based row index.
//!
//! [`Block`]: Modality::Block
//! [`Document`]: crate::document::Document
//! [`Span`]: crate::document::Span
//! [`Entity`]: crate::entity::Entity

mod audio;
mod extraction;
mod image;
mod tabular;
mod text;

use std::fmt::Debug;
use std::hash::Hash;

pub use self::audio::{Audio, AudioBlock, AudioMetadata};
pub use self::extraction::{AudioExtraction, ImageExtraction, TabularExtraction, TextExtraction};
pub use self::image::{Image, ImageBlock, ImageMetadata, PageDimensions};
pub use self::tabular::{ColumnHeader, Tabular, TabularBlock, TabularMetadata};
pub use self::text::{ContextWindow, EmbeddedDocument, Text, TextBlock, TextContent, TextMetadata};

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
/// - [`Block`] — the modality's block variant.
/// - [`Metadata`] — document-level metadata (languages, page
///   dimensions, column headers, the importer's extraction tag).
///
/// [`Block`]: Self::Block
/// [`Metadata`]: Self::Metadata
/// [`Entity<M>`]: crate::entity::Entity
/// [`Audit<M>`]: crate::provenance::Audit
pub trait Modality: Clone + Debug + PartialEq + Send + Sync + 'static {
    /// The modality's block payload. See per-modality types:
    /// [`TextBlock`], [`ImageBlock`], [`AudioBlock`], [`TabularBlock`].
    type Block: ModalityBlock + Clone + Debug + Send + Sync + 'static;

    /// Document-level metadata. Carries the importer's extraction tag
    /// ([`TextExtraction`] / [`ImageExtraction`] / [`AudioExtraction`] /
    /// [`TabularExtraction`]) plus modality-specific fields (languages,
    /// page dimensions, column headers, …). No `Default` bound: every
    /// document is imported with a known extraction path, so the
    /// importer must always supply metadata rather than rely on a
    /// placeholder default.
    type Metadata: Clone + Debug + PartialEq + Send + Sync + 'static;

    /// The modality's redaction strategy. Each modality declares the
    /// methods that make sense for its data — text picks
    /// mask/replace/encrypt/etc., image picks blur/block/pixelate,
    /// audio picks silence/remove, tabular picks clear/drop-column.
    type Strategy: RedactionStrategy + Clone + Debug + Default + PartialEq + Send + Sync + 'static;

    /// What an applied redaction wrote back at the entity's
    /// location. The shape is per-modality:
    ///
    /// - **Text / Tabular** carry the replacement string (or
    ///   "removed" for whole-span deletion) since the substitution
    ///   itself is text-shaped.
    /// - **Image / Audio** carry the method tag only — the
    ///   substitution is a binary pixel/sample transform whose
    ///   parameters live on [`Self::Strategy`]; the audit just
    ///   records *which* operation ran.
    ///
    /// Persisted on [`Execution::Applied`] as the per-entry
    /// "what we wrote" record.
    ///
    /// [`Execution::Applied`]: crate::provenance::Execution::Applied
    type Replacement: Clone + Debug + PartialEq + Send + Sync + 'static;
}

/// Shared per-block surface every modality's block payload exposes.
///
/// This is the trait every [`Modality::Block`] type implements; it
/// collects the modality-agnostic queries the engine needs to drive
/// pipeline stages without matching on the concrete block variant.
///
/// Today the only method is [`scan_text`], used by the detection
/// driver to decide whether a block carries text the text-typed
/// recognizers should scan. Future shared block queries (block id,
/// kind tag, block-level confidence accessor) land here.
///
/// `Text` and `Tabular` blocks always carry text; their impls return
/// `Some(_)` unconditionally. Image and audio impls return `None`
/// for non-text variants (figures, silences).
///
/// [`scan_text`]: Self::scan_text
pub trait ModalityBlock {
    /// The text a text-typed recognizer should scan over this block,
    /// or `None` when the block carries no scannable text (image
    /// figures/logos, audio silences).
    fn scan_text(&self) -> Option<&str>;
}

/// What a redacted output leaks about the original it replaced.
///
/// Variants are ordered from most-leaky to least-leaky, so
/// `Recoverable < Partial < Irrecoverable`. Used today for operator
/// understanding and policy authoring; future conflict-resolution
/// passes may consult the ordering when two methods compete (see
/// engine deduplication + #244).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LeakProfile {
    /// The original value is recoverable from the output given the
    /// right metadata (encryption key, token vault, pseudonym map,
    /// or the candidate entity list against a hash).
    Recoverable,
    /// The original value is gone, but observable shape leaks:
    /// position, length, bounding box, cell coordinates, or a known
    /// silence on the timeline.
    Partial,
    /// No trace of the original value or its shape remains in the
    /// output.
    Irrecoverable,
}

/// Methods every per-modality redaction strategy must expose.
pub trait RedactionStrategy {
    /// What the strategy's output leaks about the original.
    fn leak_profile(&self) -> LeakProfile;

    /// Whether the strategy is reversible — true iff the leak
    /// profile is [`Recoverable`].
    ///
    /// [`Recoverable`]: LeakProfile::Recoverable
    fn is_reversible(&self) -> bool {
        self.leak_profile() == LeakProfile::Recoverable
    }
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

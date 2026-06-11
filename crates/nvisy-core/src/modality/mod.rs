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
//! onto a document at extractor time), plus the [`KIND`] runtime tag
//! ([`ModalityKind`]) used wherever `M` has been erased. The
//! document-shape side (`Block`, `Metadata`) lives in `nvisy-engine`
//! via a `DocumentModality` extension trait — toolkit and document
//! don't pollute core.
//!
//! [`KIND`]: Modality::KIND
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

use std::any::TypeId;
use std::fmt::Debug;
use std::hash::Hash;

use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub use self::audio::{Audio, AudioData, AudioExtraction, AudioLocation};
pub use self::image::{Image, ImageData, ImageExtraction, ImageLocation};
pub use self::tabular::{Tabular, TabularExtraction, TabularLocation};
pub use self::text::{ContextWindow, Text, TextData, TextExtraction, TextLocation};

/// Runtime tag identifying which [`Modality`] a generic container
/// carries. Used wherever the typed `M` is erased — the codec
/// registry returning `UntypedDocumentHandle`, redaction
/// applicator switching on the audit modality, etc.
///
/// Closed set today (Text/Tabular/Image/Audio). Adding a fifth
/// modality is a workspace-wide change — it also requires a new
/// variant on every `Untyped*` enum and a [`Modality`] impl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
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

impl ModalityKind {
    /// Return the [`ModalityKind`] for a typed `M: Modality` at the
    /// call site.
    ///
    /// Resolved at runtime via [`TypeId`]; the match is exhaustive
    /// over the four built-in marker types. An unknown `M` panics
    /// — the modality set is closed.
    #[must_use]
    pub fn of<M: Modality>() -> Self {
        let id = TypeId::of::<M>();
        if id == TypeId::of::<Text>() {
            Self::Text
        } else if id == TypeId::of::<Tabular>() {
            Self::Tabular
        } else if id == TypeId::of::<Image>() {
            Self::Image
        } else if id == TypeId::of::<Audio>() {
            Self::Audio
        } else {
            unreachable!("Modality must be one of Text/Tabular/Image/Audio");
        }
    }
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
    /// Stable lowercase identifier for this modality (e.g.
    /// `"text"`, `"image"`). Used as the value of structured
    /// telemetry fields and as a wire-friendly modality tag in
    /// logs. Each marker advertises its own; downstream code never
    /// pattern-matches on the closed set.
    const NAME: &'static str;

    /// Runtime tag for this modality. Used wherever the typed `M`
    /// is erased — the codec registry, untyped document handles,
    /// the redaction applicator's audit dispatch, etc.
    const KIND: ModalityKind;

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
    type Data: Clone + Hash + Debug + Send + Sync;

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
    /// [`Document<M>`]: # "carrier owned by nvisy-engine"
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

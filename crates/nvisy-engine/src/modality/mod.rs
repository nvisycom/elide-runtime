//! Document-shape per-modality types: block payloads, metadata,
//! and the [`DocumentModality`] extension trait.
//!
//! Document-level shape lives here, not in `nvisy-core`. Core owns the
//! atomic [`Modality`] marker trait + the per-modality coordinate
//! structs (`Text`, `Image`, `Audio`, `Tabular`); this module adds:
//!
//! - per-modality block payloads (`TextBlock`, `ImageBlock`,
//!   `AudioBlock`, `TabularBlock`) describing what a block of that
//!   modality carries;
//! - per-modality document-level metadata (`TextMetadata`, etc.)
//!   carrying extraction tags + per-modality fields (languages, page
//!   dimensions, column headers);
//! - the [`ModalityBlock`] trait every block payload implements
//!   (shared per-block queries);
//! - the [`DocumentModality`] extension trait that binds `Block` +
//!   `Metadata` per modality and is the trait every document-shape
//!   generic actually bounds on.
//!
//! [`Modality`]: nvisy_core::modality::Modality

mod any;
mod audio;
mod image;
mod tabular;
mod text;

use std::fmt::Debug;

pub use nvisy_codec::core::Codable;
pub use nvisy_core::modality::{
    Audio, AudioExtraction, Image, ImageExtraction, Modality, Tabular, TabularExtraction, Text,
    TextExtraction,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub use self::any::AnyLocation;
pub use self::audio::{AudioBlock, AudioMetadata};
pub use self::image::{ImageBlock, ImageMetadata, PageDimensions};
pub use self::tabular::{ColumnHeader, TabularBlock, TabularMetadata};
pub use self::text::{EmbeddedDocument, TextBlock, TextContent, TextMetadata};
use crate::policy::redaction::{AudioRedaction, ImageRedaction, TabularRedaction, TextRedaction};

/// Shared per-block surface every modality's block payload exposes.
///
/// This is the trait every [`DocumentModality::Block`] type
/// implements; it collects the modality-agnostic queries the engine
/// needs to drive pipeline stages without matching on the concrete
/// block variant.
///
/// Today the only method is [`scan_text`], used by the detection
/// driver to decide whether a block carries text the text-typed
/// recognizers should scan. Future shared block queries (block id,
/// kind tag, block-level confidence accessor) land here.
///
/// [`scan_text`]: Self::scan_text
pub trait ModalityBlock {
    /// The text a text-typed recognizer should scan over this block,
    /// or `None` when the block carries no scannable text (image
    /// figures/logos, audio silences).
    fn scan_text(&self) -> Option<&str>;
}

/// Document-shape extension of [`Modality`] + [`Codable`].
///
/// Adds the document-level shape (block payload, document metadata,
/// policy redaction enum) the document carrier and its phases need.
/// Implemented for the four modalities in this module — code that
/// just needs a marker can bound on bare `Modality`; code that
/// needs any document-level associated type bounds on
/// `DocumentModality`.
///
/// The [`Codable`] super-trait carries the codec-side tag the
/// pipeline needs to drive read/write through a handler. Folding it
/// into `DocumentModality` lets every downstream site spell its bound
/// as `M: DocumentModality` and inherit both axes automatically.
pub trait DocumentModality: Modality + Codable {
    /// The modality's block payload. See [`TextBlock`], [`ImageBlock`],
    /// [`AudioBlock`], [`TabularBlock`].
    type Block: ModalityBlock + Clone + Debug + Send + Sync + 'static;

    /// Document-level metadata: extraction tag plus modality-specific
    /// fields (languages, page dimensions, column headers).
    type Metadata: Clone + Debug + PartialEq + Send + Sync + 'static;

    /// Operator-spec enum a `redact` policy rule of this modality
    /// carries. Per-call instantiated for built-ins; `Custom(id)`
    /// arms resolve through the toolkit-side
    /// [`RedactionRegistry<Self>`].
    ///
    /// The serde + schemars bounds let [`Policy<Self>`] /
    /// [`PolicyRule<Self>`] derive Serialize / Deserialize /
    /// JsonSchema transparently across all four modalities — the
    /// per-modality enum types meet them naturally.
    ///
    /// [`RedactionRegistry<Self>`]: nvisy_toolkit::redaction::RedactionRegistry
    /// [`Policy<Self>`]: crate::policy::Policy
    /// [`PolicyRule<Self>`]: crate::policy::PolicyRule
    type Redaction: Clone
        + Debug
        + PartialEq
        + Send
        + Sync
        + Serialize
        + DeserializeOwned
        + JsonSchema
        + 'static;
}

impl DocumentModality for Text {
    type Block = TextBlock;
    type Metadata = TextMetadata;
    type Redaction = TextRedaction;
}

impl DocumentModality for Image {
    type Block = ImageBlock;
    type Metadata = ImageMetadata;
    type Redaction = ImageRedaction;
}

impl DocumentModality for Audio {
    type Block = AudioBlock;
    type Metadata = AudioMetadata;
    type Redaction = AudioRedaction;
}

impl DocumentModality for Tabular {
    type Block = TabularBlock;
    type Metadata = TabularMetadata;
    type Redaction = TabularRedaction;
}

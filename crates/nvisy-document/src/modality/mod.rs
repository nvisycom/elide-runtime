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

mod audio;
pub mod extraction;
mod image;
mod tabular;
mod text;

use std::fmt::Debug;

pub use nvisy_core::modality::{Audio, Image, Modality, Tabular, Text};

pub use self::audio::{AudioBlock, AudioMetadata};
pub use self::extraction::{AudioExtraction, ImageExtraction, TabularExtraction, TextExtraction};
pub use self::image::{ImageBlock, ImageMetadata, PageDimensions};
pub use self::tabular::{ColumnHeader, TabularBlock, TabularMetadata};
pub use self::text::{EmbeddedDocument, TextBlock, TextContent, TextMetadata};

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

/// Document-shape extension of [`Modality`].
///
/// Adds the document-level shape (block payload, document metadata)
/// the document carrier and its phases need. Implemented for the
/// four modalities in this module — code that just needs a marker
/// can bound on bare `Modality`; code that needs `M::Block` or
/// `M::Metadata` bounds on `DocumentModality`.
pub trait DocumentModality: Modality {
    /// The modality's block payload. See [`TextBlock`], [`ImageBlock`],
    /// [`AudioBlock`], [`TabularBlock`].
    type Block: ModalityBlock + Clone + Debug + Send + Sync + 'static;

    /// Document-level metadata: extraction tag plus modality-specific
    /// fields (languages, page dimensions, column headers).
    type Metadata: Clone + Debug + PartialEq + Send + Sync + 'static;
}

impl DocumentModality for Text {
    type Block = TextBlock;
    type Metadata = TextMetadata;
}

impl DocumentModality for Image {
    type Block = ImageBlock;
    type Metadata = ImageMetadata;
}

impl DocumentModality for Audio {
    type Block = AudioBlock;
    type Metadata = AudioMetadata;
}

impl DocumentModality for Tabular {
    type Block = TabularBlock;
    type Metadata = TabularMetadata;
}

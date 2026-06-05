//! [`ModalityExtraction`]: extension trait that names a modality's
//! per-call provenance enum.
//!
//! Lives in `extraction/` (alongside [`Extractor`]) because the value
//! it adds is purely an extractor-time concern — the modality marker
//! itself stays primitive in [`crate::modality`]. Each per-modality
//! `*Extraction` enum (the concrete value bound to
//! `M::Extraction`) lives next to its modality marker, since the
//! enum's variants are modality-specific.
//!
//! [`Extractor`]: crate::extraction::Extractor

use std::fmt::Debug;

use crate::modality::{
    Audio, AudioExtraction, Image, ImageExtraction, Modality, Tabular, TabularExtraction, Text,
    TextExtraction,
};

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

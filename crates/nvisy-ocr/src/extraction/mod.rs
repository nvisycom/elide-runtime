//! Extraction layer: the [`OcrExtractor`] that drives any
//! [`OcrBackend`] backend through a type-erased
//! [`Arc<dyn OcrBackend>`] slot.
//!
//! Implements [`Extractor<Image>`] (from `nvisy-core`) so it
//! composes with the rest of the platform through the same trait
//! every other image extractor uses.
//!
//! [`OcrBackend`]: crate::backend::OcrBackend
//! [`Extractor<Image>`]: nvisy_core::extraction::Extractor

mod extractor;

pub use self::extractor::OcrExtractor;

//! Extraction: the [`ExtractorRegistry`] plus per-backend re-exports.
//!
//! Toolkit owns the registry side of extraction — typed slots for one
//! per-modality extractor each. The trait the registry holds
//! ([`Extractor`]) lives in `nvisy-core` so backend crates can
//! implement it without depending on toolkit. Each backend crate is
//! re-exported here under its own submodule ([`ocr`] and [`stt`]) so a
//! consumer that wants the shipped backends only needs to depend on
//! `nvisy-toolkit`.
//!
//! TOML-deserialisable backend selectors and config bundles
//! (`OcrBackend`, `OcrExtractorConfig`, `SttExtractorConfig`) live in
//! `nvisy-document`'s `pipeline::config::extraction` — they're the
//! glue that turns config into concrete `Arc<dyn Extractor<M>>`
//! instances that get inserted into [`ExtractorRegistry`].
//!
//! [`Extractor`]: nvisy_core::extraction::Extractor

#[cfg(feature = "image")]
pub mod ocr;
pub mod registry;
#[cfg(feature = "audio")]
pub mod stt;

pub use nvisy_core::extraction::Extractor;

#[cfg(feature = "audio")]
pub use self::registry::AudioExtractorOutput;
pub use self::registry::ExtractorRegistry;
#[cfg(feature = "image")]
pub use self::registry::ImageExtractorOutput;

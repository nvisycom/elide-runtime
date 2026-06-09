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
//! TOML-deserialisable backend selectors and config bundles live in
//! a higher pipeline layer that turns config into concrete
//! `Arc<dyn Extractor<M>>` instances inserted into
//! [`ExtractorRegistry`].
//!
//! [`Extractor`]: nvisy_core::extraction::Extractor

#[cfg(feature = "image")]
pub mod ocr;
#[cfg(feature = "audio")]
pub mod stt;

mod registry;

pub use nvisy_core::extraction::Extractor;

#[cfg(feature = "image")]
pub use self::ocr::ImageExtractorOutput;
pub use self::registry::ExtractorRegistry;
#[cfg(feature = "audio")]
pub use self::stt::AudioExtractorOutput;

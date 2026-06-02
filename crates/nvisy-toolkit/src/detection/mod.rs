//! Detection: the [`RecognizerRegistry`] plus per-backend re-exports.
//!
//! Toolkit owns the registry side of detection — typed lists of
//! `Arc<dyn EntityRecognizer<M>>` per modality. The trait the registry
//! holds ([`nvisy_core::EntityRecognizer`]) lives in `nvisy-core` so
//! backend crates can implement it without depending on toolkit. Each
//! backend crate is re-exported here under its own submodule ([`ner`]
//! and [`pattern`]) so a consumer that wants the shipped recognizers
//! only needs to depend on `nvisy-toolkit`.
//!
//! TOML-deserialisable backend selectors and config bundles
//! (`NerBackend`, `NerDetection`, `PatternDetection`,
//! `DetectionConfig`) live in `nvisy-document`'s
//! `pipeline::config::detection` — they're the glue that turns config
//! into concrete `Arc<dyn EntityRecognizer<M>>` instances that get
//! inserted into [`RecognizerRegistry`].

pub mod ner;
pub mod pattern;
mod registry;

pub use nvisy_core::EntityRecognizer;

pub use self::registry::RecognizerRegistry;

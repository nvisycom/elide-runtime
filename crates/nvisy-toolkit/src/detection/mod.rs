//! Detection: the [`RecognizerRegistry`] plus per-backend re-exports.
//!
//! Toolkit owns the registry side of detection — typed lists of
//! `Arc<dyn EntityRecognizer<M>>` per modality. The trait the registry
//! holds ([`EntityRecognizer`]) lives in `nvisy-core` so backend crates
//! can implement it without depending on toolkit. Each backend crate
//! is re-exported here under its own submodule so a consumer that
//! wants the shipped recognizers only needs to depend on
//! `nvisy-toolkit`.
//!
//! Three backend submodules ship today:
//!
//! - [`pattern`] — `nvisy_pattern`: regex + dictionary rules.
//! - [`ner`] — `nvisy_ner`: zero-shot and adapter NER recognizers.
//! - [`llm`] — `nvisy_llm`: LLM-driven recognizers.
//!
//! TOML-deserialisable backend selectors and config bundles
//! (`NerBackend`, `NerDetection`, `PatternDetection`,
//! `DetectionConfig`) live in `nvisy-engine`'s
//! `pipeline::config::detection` — they're the glue that turns config
//! into concrete `Arc<dyn EntityRecognizer<M>>` instances that get
//! inserted into [`RecognizerRegistry`].
//!
//! [`EntityRecognizer`]: nvisy_core::recognition::EntityRecognizer

pub mod llm;
pub mod ner;
pub mod pattern;
mod registry;

pub use nvisy_core::recognition::EntityRecognizer;

pub use self::registry::RecognizerRegistry;

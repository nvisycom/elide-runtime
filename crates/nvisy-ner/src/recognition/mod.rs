//! Recognizer adapters and their configuration.
//!
//! Two shapes ship:
//!
//! - [`NlpRecognizer`] — dumb adapter. Reads
//!   [`NlpArtifacts.ner`]
//!   that an upstream [`NlpEngine`]
//!   already produced; normalizes through [`LabelMap`] +
//!   [`NerModelConfiguration`] and emits entities. Requires the
//!   orchestrator to run an `NlpEngine` first.
//! - [`GlinerRecognizer`] — zero-shot bypass. Ignores shared
//!   artifacts, calls a [`GlinerBackend`]
//!   directly with a requested-kinds list. Used when the backend
//!   is itself a zero-shot model (the externalised
//!   `inference-gliner` Bento) and doesn't fit the
//!   tokenizer→adapter pattern.
//!
//! Both implement [`Recognizer<Text>`] so
//! the engine treats them uniformly with every other text
//! recognizer (pattern, future LLM).
//!
//! [`NlpArtifacts.ner`]: nvisy_core::nlp::NlpArtifacts::ner
//! [`NlpEngine`]: crate::nlp::NlpEngine
//! [`GlinerBackend`]: crate::backend::GlinerBackend
//! [`Recognizer<Text>`]: nvisy_core::Recognizer

mod config;
mod gliner_recognizer;
mod label_map;
mod nlp_recognizer;

pub use self::config::NerModelConfiguration;
pub use self::gliner_recognizer::GlinerRecognizer;
pub use self::label_map::LabelMap;
pub use self::nlp_recognizer::NlpRecognizer;

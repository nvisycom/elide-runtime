//! Producer side of the shared-NLP-pass primitive.
//!
//! Consumer-side types ([`Tokens`], [`LanguageDetections`], …) live
//! in [`nvisy_core::nlp`] so any text consumer (pattern recognizers,
//! NER adapters, context enhancer) can read them without depending
//! on this crate. This module declares the [`NlpEngine`] trait and
//! the engines that produce those artifacts into the shared
//! `TypeMap` stamped on `TextData.artifacts`.
//!
//! One engine ships today:
//! - [`LinguaNlpEngine`] — language-only NLP, backed by the
//!   [`lingua`] crate. Used by pattern-only pipelines that still
//!   want a resolved language carried on the artifact.
//!
//! NER lives in a separate trait-driven path ([`NerBackend`] +
//! [`NerRecognizer`]) — zero-shot and fixed-label NER backends plug
//! in there, not here.
//!
//! The trait is async because realistic implementations are
//! HTTP-bound or otherwise yield.
//!
//! [`Tokens`]: nvisy_core::nlp::Tokens
//! [`LanguageDetections`]: nvisy_core::nlp::LanguageDetections
//! [`lingua`]: https://crates.io/crates/lingua
//! [`NerBackend`]: crate::backend::NerBackend
//! [`NerRecognizer`]: crate::NerRecognizer

mod capabilities;
mod engine;
mod lingua_detector;
mod lingua_engine;

pub use self::capabilities::NlpCapabilities;
pub use self::engine::NlpEngine;
pub use self::lingua_detector::LinguaDetector;
pub use self::lingua_engine::LinguaNlpEngine;

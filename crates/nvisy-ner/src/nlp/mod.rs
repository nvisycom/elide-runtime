//! Producer side of the shared-NLP-pass primitive.
//!
//! The consumer-side types ([`NlpArtifacts`], [`Tokens`], …) live in
//! [`nvisy_core::nlp`] so any text consumer (pattern recognizers,
//! NER adapters, context enhancer) can read them without depending
//! on this crate. This module declares the [`NlpEngine`] trait and
//! the engines that produce those artifacts.
//!
//! One engine ships today:
//! - [`LinguaNlpEngine`] — language-only NLP, backed by the
//!   [`lingua`] crate. Used by
//!   pattern-only pipelines that still want a resolved language
//!   carried on the artifact.
//!
//! The externalised Bento `inference-gliner` service is zero-shot
//! (takes per-call kinds, returns spans) and therefore plugs in as
//! a [`GlinerBackend`] +
//! [`GlinerRecognizer`]
//! pair, not as an [`NlpEngine`]. Future *fixed-label* NER
//! backends — a BERT-NER token classifier loaded via `ort`,
//! Candle-loaded models, or an externalised non-zero-shot
//! inference service — implement [`NlpEngine`] directly so their
//! output feeds the [`NlpRecognizer`]
//! adapter.
//!
//! The trait is async because realistic implementations are
//! HTTP-bound or otherwise yield.
//!
//! [`NlpArtifacts`]: nvisy_core::nlp::NlpArtifacts
//! [`Tokens`]: nvisy_core::nlp::Tokens
//! [`lingua`]: https://crates.io/crates/lingua
//! [`GlinerBackend`]: crate::backend::GlinerBackend
//! [`GlinerRecognizer`]: crate::recognition::GlinerRecognizer
//! [`NlpRecognizer`]: crate::recognition::NlpRecognizer

mod engine;
mod lingua_detector;
mod lingua_engine;

pub use self::engine::NlpEngine;
pub use self::lingua_detector::LinguaDetector;
pub use self::lingua_engine::LinguaNlpEngine;

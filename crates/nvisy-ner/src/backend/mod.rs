//! Transport layer for direct-model recognizers.
//!
//! Two-shape backend surface, mirroring Presidio's
//! `RemoteRecognizer` escape hatch:
//!
//! - [`GlinerBackend`] is the trait. Zero-shot NER backends
//!   implement it; the runtime hands them a text + a requested
//!   `EntityKind` allowlist and gets back classified spans
//!   directly. No shared NLP artifacts in either direction —
//!   zero-shot models don't benefit from an upstream tokenizer
//!   pass and they take per-call kinds as input.
//! - Built-in impls live in this module: [`NoopBackend`] (returns
//!   no entities; baseline) and [`BentoBackend`] (HTTP call into
//!   the externalised `inference-gliner` service; feature `bento`).
//!
//! The complement — for backends that *do* fit the
//! tokenizer→NER-adapter pattern — is the [`NlpEngine`] trait in
//! [`crate::nlp`]. Use that path for fixed-label classifiers
//! (BERT-NER over ONNX/`ort`, Candle-loaded token classifiers, an
//! externalised non-zero-shot inference service); use this path
//! for zero-shot APIs where the caller picks the kinds per call.
//!
//! [`NlpEngine`]: crate::nlp::NlpEngine

#[cfg(feature = "bento")]
mod bento_backend;
#[cfg(feature = "bento")]
mod bento_types;
mod gliner_backend;
mod noop_backend;

#[cfg(feature = "bento")]
#[cfg_attr(docsrs, doc(cfg(feature = "bento")))]
pub use self::bento_backend::{BentoBackend, BentoParams};
pub use self::gliner_backend::{GlinerBackend, GlinerRequest};
pub use self::noop_backend::NoopBackend;

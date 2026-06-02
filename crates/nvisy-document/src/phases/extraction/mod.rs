//! Extraction: shared extractor registry + per-recognizer-kind
//! backend modules.
//!
//! Public surface is the [`ExtractionEngine`] re-exported below
//! plus the per-backend extractor types. The phase orchestrator
//! that walks a [`DocumentTree`] and drives the engine lives in
//! [`ExtractionPhase`].
//!
//! Per-backend modules host the technique implementations:
//!
//! - `ocr` — image OCR backend (`image` cargo feature).
//! - `stt` — audio STT backend (`audio` cargo feature).
//!
//! [`DocumentTree`]: crate::core::DocumentTree
//! [`ExtractionPhase`]: crate::pipeline::ExtractionPhase

mod engine;
#[cfg(feature = "image")]
mod ocr;
pub mod phase;
#[cfg(feature = "audio")]
mod stt;

pub use self::engine::ExtractionEngine;
#[cfg(feature = "image")]
pub use self::ocr::OcrExtractor;
#[cfg(feature = "audio")]
pub use self::stt::SttExtractor;

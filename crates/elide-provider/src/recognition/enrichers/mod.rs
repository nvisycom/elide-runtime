//! Enrichers: the components that produce context for recognizers.
//!
//! One module per backend, each holding its deployment
//! configuration. [`compile`] turns those lineups into the
//! enrichers elide runs.
//!
//! Enrichers run before recognition and stamp side-channel data —
//! a language hint, OCR'd text layout, audio transcript segments —
//! onto the per-request context, so a recognizer downstream reads
//! what they found. Language detection has no configuration (elide
//! wires lingua unconditionally) so it appears only in [`compile`].

mod ocr;
mod stt;

pub(crate) mod compile;

pub use self::ocr::{OcrBackend, OcrConfig};
pub use self::stt::{SttBackend, SttConfig};

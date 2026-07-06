//! Per-enricher compile helpers: `language`, `ocr`, `stt`.
//!
//! Symmetric with [`super::recognizer`]. Enrichers run before
//! recognition and stamp side-channel data (language hint,
//! OCR'd text layout, audio transcript segments) onto the
//! per-request context so downstream recognizers pick it up
//! transparently.
//!
//! Every enricher is at-most-one per analyzer; the caller-facing
//! spec is the corresponding slot on
//! [`nvisy_schema::plan::EnricherParams`].

mod language;
mod ocr;
mod stt;

pub(super) use self::language::attach as attach_language;
pub(super) use self::ocr::attach as attach_ocr;
pub(super) use self::stt::attach as attach_stt;

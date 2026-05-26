//! Wire types for the externalised `inference-ocr` Bento.
//!
//! **Scaffolding only — the wire contract has not been finalised
//! in [`nvisycom/inference`].** The types below are placeholders so
//! `BentoOcrBackend` can compile and round-trip through serde, but
//! the real shape will be defined alongside the service and pulled
//! in here once that work lands.
//!
//! When the contract finalises, this module will mirror
//! `nvisy_core.ocr.v1` the same way `nvisy-ner`'s `backend/bento_types`
//! mirrors `nvisy_core.ner.v1`: each struct documented in place,
//! field names camelCase on the wire, schema version baked into
//! the module name.
//!
//! [`nvisycom/inference`]: https://github.com/nvisycom/inference

// TODO(#128): mirror `nvisy_core.ocr.v1` here once the inference
// repo finalises the OCR wire contract. Until then `BentoOcrBackend`
// returns a clear runtime error from its trait methods rather than
// pretending to call the service.

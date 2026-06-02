//! Re-export the [`nvisy_ocr`] backend surface as
//! `nvisy_toolkit::extraction::ocr`.
//!
//! A consumer that wants the shipped OCR backends only needs the
//! `nvisy-toolkit` dep — `nvisy_toolkit::extraction::ocr::Extractor`,
//! `nvisy_toolkit::extraction::ocr::Backend`,
//! `nvisy_toolkit::extraction::ocr::NoopBackend`, etc. are all
//! reachable here.

pub use nvisy_ocr::*;

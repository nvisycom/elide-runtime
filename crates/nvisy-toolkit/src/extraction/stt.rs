//! Re-export the [`nvisy_stt`] surface as
//! `nvisy_toolkit::extraction::stt`.
//!
//! A consumer that wants the shipped STT backends only needs the
//! `nvisy-toolkit` dep — `nvisy_toolkit::extraction::stt::SttExtractor`,
//! `nvisy_toolkit::extraction::stt::NoopBackend`, etc. are all
//! reachable here.

pub use nvisy_stt::*;

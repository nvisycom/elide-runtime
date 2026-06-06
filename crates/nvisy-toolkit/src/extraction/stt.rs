//! Re-export the [`stt`] backend surface as
//! `nvisy_toolkit::extraction::stt`.
//!
//! A consumer that wants the shipped STT backends only needs the
//! `nvisy-toolkit` dep — `nvisy_toolkit::extraction::stt::SttService`,
//! `nvisy_toolkit::extraction::stt::SttProvider`, etc. are all
//! reachable here.
//!
//! [`stt`]: nvisy_agent::audio::stt

pub use nvisy_agent::audio::stt::*;

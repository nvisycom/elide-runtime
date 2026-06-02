//! Re-export the [`nvisy_pattern`] backend surface as
//! `nvisy_toolkit::detection::pattern`.
//!
//! A consumer that wants the shipped pattern recognizers only needs
//! the `nvisy-toolkit` dep —
//! `nvisy_toolkit::detection::pattern::PatternRecognizer`,
//! `nvisy_toolkit::detection::pattern::patterns::all()`, etc. are all
//! reachable here.

pub use nvisy_pattern::*;

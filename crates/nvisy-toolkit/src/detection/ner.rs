//! Re-export the [`nvisy_ner`] backend surface as
//! `nvisy_toolkit::detection::ner`.
//!
//! A consumer that wants the shipped NER backends only needs the
//! `nvisy-toolkit` dep — `nvisy_toolkit::detection::ner::backend::Backend`,
//! `nvisy_toolkit::detection::ner::backend::NoopBackend`, etc. are
//! all reachable here.

pub use nvisy_ner::*;

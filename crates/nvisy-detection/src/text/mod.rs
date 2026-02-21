//! Text detection layers.

pub mod pattern;
pub mod ner;

pub use pattern::{PatternDetection, PatternDetectionParams};
pub use ner::{NerDetection, NerDetectionParams, NerBackend, NerConfig, parse_ner_entities};

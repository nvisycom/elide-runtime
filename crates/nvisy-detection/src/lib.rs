#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod ontology;
mod layer;
mod text;
mod tabular;
mod document;
pub mod image;

pub mod prelude;

// Re-export domain types from nvisy-core for convenience
pub use nvisy_core::data::{EntityCategory, EntityKind, EntitySensitivity, LayoutKind};

// Domain types
pub use ontology::*;

// Detection traits
pub use layer::*;

// Detection layers
pub use text::{PatternDetection, PatternDetectionParams};
pub use text::{NerDetection, NerDetectionParams, NerBackend, NerConfig, parse_ner_entities};
pub use tabular::{TabularDetection, TabularDetectionParams, ColumnRule};

// Standalone actions
pub use document::{DetectManualAction, DetectManualParams, Exclusion, ManualOutput, is_excluded};
pub use document::DeduplicateAction;

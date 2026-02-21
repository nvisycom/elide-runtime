#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

// Domain types
mod entity;
mod location;
mod model;
mod result;
mod selector;
mod annotation;

// Detection logic
mod context;
mod layer;
mod text;
mod tabular;
mod document;

// Re-export pattern and dictionary infrastructure from nvisy-pattern
pub use nvisy_pattern::{registry, patterns, dictionaries, validators};

// Re-export domain types from nvisy-core for convenience
pub use nvisy_core::data::{EntityCategory, EntityKind, EntitySensitivity, LayoutKind};

// Domain types
pub use entity::{DetectionMethod, Entity};
pub use location::{
    AudioLocation, ImageLocation, TabularLocation, TextLocation, VideoLocation,
};
pub use model::{ModelInfo, ModelKind};
pub use result::DetectionResult;
pub use selector::EntitySelector;
pub use annotation::{Annotation, AnnotationKind, AnnotationLabel, AnnotationScope};

// Detection traits
pub use context::{DetectionContext, ParallelContext, SequentialContext};
pub use layer::{DetectionLayer, Detect};

// Detection layers
pub use text::{PatternDetection, PatternDetectionParams};
pub use text::{DictionaryDetection, DictionaryDetectionParams, DictionaryDef};
pub use text::{NerDetection, NerDetectionParams, NerBackend, NerConfig, parse_ner_entities};
pub use tabular::{TabularDetection, TabularDetectionParams, ColumnRule};

// Standalone actions
pub use document::{DetectChecksumAction, DetectChecksumParams};
pub use document::{DetectManualAction, DetectManualParams};
